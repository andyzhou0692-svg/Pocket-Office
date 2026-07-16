//! `HookRouter` — the single honest owner of the ONE shared hook socket that
//! EVERY source's hooks ride (Codex, Reasonix, CodeWhale, opencode, Cursor, and
//! the OpenClaw daemon). It used to be bound as a side-effect of
//! `ClaudeCodeSource` (a transcript watcher that "happened to" host the socket);
//! lifting it here makes CC a pure `JsonlWatcher` and gives the hook plane a
//! dedicated owner.
//!
//! It implements [`Source`] DELIBERATELY: `Source::run` already hands the one
//! `(Transport, AgentEvent)` sender `handle_conn` sends on, and
//! `SourceManager::spawn_with_health` *generates* the `SourceDeath` that surfaces
//! a fatal exit in the TUI footer (#157) — so the router inherits that path for
//! free instead of rebuilding it. It is INFRASTRUCTURE, not a CLI: it has NO
//! `REGISTERED_SOURCES`/descriptor/badge row (the add-a-CLI checklist does not
//! apply); `source_set_includes_the_hook_router` closes the spawned-but-untested
//! gap.

use anyhow::Result;

use crate::source::jsonl::ChildEndUnclaims;
use crate::source::{AgentEvent, Source, TaggedReceiver, TaggedSender, Transport};

use super::{HookPidWatch, HookSocketListener, PresenceSender, SocketBusy};

/// Infrastructure name — NOT a registered source (no descriptor/badge). Used by
/// `spawn_with_health` to attribute a fatal listener exit in the footer.
pub(crate) const SOURCE_NAME: &str = "hook-router";

/// Producer half of the #246 child-end un-claim side-channel (see
/// `ChildEndUnclaims` for the WHY). Interposed between the hook listener and the
/// real channel inside [`HookRouter::run`] — the listener's API stays
/// source-agnostic; this is the ONE seam every decoded `SubagentStop` (CC and
/// Codex alike — all sources' hooks ride the one shared socket) passes through.
/// Every event is forwarded UNCHANGED, transport tag included (invariant #2: the
/// producer's tag flows through). The push happens BEFORE the forward — the
/// order is irrelevant for correctness (the watcher drains on its own scan
/// cadence), but push-first means the un-claim is already pending by the time
/// the reducer applies the end, which keeps tests deterministic. Exits when
/// either side closes (listener gone → `recv` None; reducer gone → send Err).
pub(crate) async fn tee_child_end_unclaims(
    mut rx: TaggedReceiver,
    tx: TaggedSender,
    unclaims: ChildEndUnclaims,
) {
    while let Some((transport, ev)) = rx.recv().await {
        if transport == Transport::Hook {
            if let AgentEvent::SessionEnd {
                agent_id,
                as_child: true,
            } = &ev
            {
                unclaims.push(*agent_id);
            }
        }
        if tx.send((transport, ev)).await.is_err() {
            return;
        }
    }
}

/// The shared hook-socket owner. Binds the ONE socket, runs the per-connection
/// decode loop, interposes the #246 tee, and feeds the daemon-presence side
/// channel. Builder fields mirror the old `ClaudeCodeSource` plumbing exactly —
/// they just live on their honest owner now.
enum HookEndpoint {
    Pending(std::path::PathBuf),
    Bound(HookSocketListener),
}

pub struct HookRouter {
    endpoint: HookEndpoint,
    /// The #246 child-end un-claim PRODUCER handle (the tee). The runtime shares
    /// ONE handle with the CC + Codex watchers (the CONSUMERS). `None` disables
    /// the tee (bare test construction).
    child_end_unclaims: Option<ChildEndUnclaims>,
    /// The daemon-presence side channel (the gateway mascots). Daemon payloads
    /// decode to presence deltas (no `AgentEvent`s) routed here for the reducer
    /// task. `None` disables it (bare test construction).
    presence_tx: Option<PresenceSender>,
}

impl HookRouter {
    /// Construct a router that binds when its source task starts. Kept for API
    /// and behavior compatibility, including its legacy quiet exit when another
    /// owner already holds the endpoint. Application startup uses
    /// [`HookRouter::bind`] so a duplicate is rejected before any renderer opens.
    pub fn new(socket_path: std::path::PathBuf) -> Self {
        Self {
            endpoint: HookEndpoint::Pending(socket_path),
            child_end_unclaims: None,
            presence_tx: None,
        }
    }

    /// Claim the shared hook endpoint before any renderer starts. Keeping the
    /// bound listener inside the router makes endpoint ownership a single,
    /// race-free startup decision instead of a later source-task side effect.
    pub async fn bind(socket_path: impl Into<std::path::PathBuf>) -> Result<Self> {
        Ok(Self {
            endpoint: HookEndpoint::Bound(HookSocketListener::bind(socket_path).await?),
            child_end_unclaims: None,
            presence_tx: None,
        })
    }

    /// Wire the #246 child-end un-claim producer (the driver passes the shared
    /// handle whose consumers are the CC + Codex watchers).
    pub fn with_child_end_unclaims(mut self, unclaims: Option<ChildEndUnclaims>) -> Self {
        self.child_end_unclaims = unclaims;
        self
    }

    /// Wire the daemon-presence side channel (the driver passes the sender).
    pub fn with_presence_tx(mut self, presence_tx: Option<PresenceSender>) -> Self {
        self.presence_tx = presence_tx;
        self
    }
}

impl Source for HookRouter {
    fn name(&self) -> &str {
        SOURCE_NAME
    }

    async fn run(self: Box<Self>, tx: TaggedSender) -> Result<()> {
        let socket = match self.endpoint {
            HookEndpoint::Pending(path) => match HookSocketListener::bind(path).await {
                Ok(socket) => socket,
                Err(error) if error.downcast_ref::<SocketBusy>().is_some() => {
                    tracing::warn!("{error:#}; legacy HookRouter::new owner stays inactive");
                    return Ok(());
                }
                Err(error) => return Err(error),
            },
            HookEndpoint::Bound(socket) => socket,
        };
        // #246: route hook events through the un-claim tee when the side-channel
        // is wired (the runtime always wires it; `None` is bare test
        // construction). The tee task is a passive pipe and dies with the
        // listener (its sender drops).
        let tx_hook = match &self.child_end_unclaims {
            Some(unclaims) => {
                // Same capacity as the runtime's event channel: the tee adds a
                // stage, not a different backpressure policy.
                let (tee_tx, tee_rx) =
                    tokio::sync::mpsc::channel(crate::source::EVENT_CHANNEL_CAPACITY);
                tokio::spawn(tee_child_end_unclaims(tee_rx, tx.clone(), unclaims.clone()));
                tee_tx
            }
            None => tx.clone(),
        };
        // Hook-supplied-pid liveness (CodeWhale / opencode): their hooks ride
        // THIS shared socket. The synthesized SessionEnd goes on the main `tx`
        // (it is `as_child: false`, so the #246 tee — which acts only on
        // `as_child: true` — ignores it anyway). `None` on platforms without an
        // exit-watch backend → no-op.
        let socket = socket
            .with_pid_watch(HookPidWatch::spawn(tx.clone()))
            .with_presence(self.presence_tx.clone());
        socket.run(tx_hook).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentId;
    use std::time::Duration;

    /// The #246 tee contract: a Hook-transport `SessionEnd { as_child: true }`
    /// flowing through the forwarding task lands its id in the shared handle,
    /// AND every event — that one included — reaches the downstream channel
    /// UNCHANGED, in order, transport tag intact (invariant #2). Jsonl-tagged
    /// child ends and root (`as_child: false`) hook ends must NOT be pushed:
    /// copilot's transcript decode emits Jsonl child ends
    /// (`subagent.completed`/`failed`), so the transport guard is load-bearing,
    /// not merely defensive.
    #[tokio::test]
    async fn tee_pushes_hook_child_ends_and_forwards_every_event_unchanged() {
        let unclaims = ChildEndUnclaims::new();
        let (in_tx, in_rx) = tokio::sync::mpsc::channel(16);
        let (out_tx, mut out_rx) = tokio::sync::mpsc::channel(16);
        let tee = tokio::spawn(tee_child_end_unclaims(in_rx, out_tx, unclaims.clone()));

        let child = AgentId::from_parts("codex", "child-uuid");
        let root = AgentId::from_parts(crate::source::claude_code::SOURCE_NAME, "root-uuid");
        let events: Vec<(Transport, AgentEvent)> = vec![
            (
                Transport::Hook,
                AgentEvent::ActivityStart {
                    agent_id: root,
                    tool_use_id: Some("tu_1".into()),
                    detail: None,
                },
            ),
            // A JSONL-tagged child end must not feed the handle — copilot's
            // transcript decode emits exactly this shape (`subagent.completed`
            // / `failed` lines), so the transport guard IS the boundary.
            (
                Transport::Jsonl,
                AgentEvent::SessionEnd {
                    agent_id: root,
                    as_child: true,
                },
            ),
            // A root hook end is not a SubagentStop — not pushed.
            (
                Transport::Hook,
                AgentEvent::SessionEnd {
                    agent_id: root,
                    as_child: false,
                },
            ),
            // THE shape: the decoded SubagentStop.
            (
                Transport::Hook,
                AgentEvent::SessionEnd {
                    agent_id: child,
                    as_child: true,
                },
            ),
        ];
        for ev in &events {
            in_tx.send(ev.clone()).await.unwrap();
        }
        for expected in &events {
            let got = tokio::time::timeout(Duration::from_secs(5), out_rx.recv())
                .await
                .expect("tee must forward promptly")
                .expect("tee must not drop the channel");
            assert_eq!(
                &got, expected,
                "event parity: forwarded unchanged, in order"
            );
        }
        assert_eq!(
            unclaims.take_matching(|_| true),
            vec![child],
            "exactly the Hook-transport as_child end lands in the handle"
        );
        drop(in_tx);
        tokio::time::timeout(Duration::from_secs(5), tee)
            .await
            .expect("tee exits when the listener side closes")
            .unwrap();
    }

    /// `name()` is the footer attribution for a fatal listener exit
    /// (`spawn_with_health` publishes `SourceDeath { source: name }`) — a blank
    /// or drifted name makes that death message unattributable.
    #[test]
    fn router_name_is_the_infrastructure_source_name() {
        let router = HookRouter::new(std::path::PathBuf::from("unused.sock"));
        assert_eq!(router.name(), SOURCE_NAME);
    }

    /// Bind failures surface during construction, before the router can be
    /// spawned and before any renderer opens.
    #[cfg(unix)]
    #[tokio::test]
    async fn fatal_bind_error_is_a_startup_error() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("no-such-dir").join("hook.sock");
        let res = HookRouter::bind(bad).await;
        assert!(res.is_err(), "a bind failure must abort startup");
    }
}
