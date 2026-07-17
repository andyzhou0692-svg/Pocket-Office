//! The `native`-only runtime half of the Codex source: the liveness probe
//! (open-rollout FD binding plus desktop turn-state filtering) + `CodexSource`
//! and its `JsonlWatcher` wiring.
//! The pure decoder stays in the always-compiled parent module; this whole
//! file sits behind the parent's ONE `#[cfg(feature = "native")] mod native;`
//! gate and is re-exported there, so public paths don't move.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;

use super::{
    codex_home, codex_id_from_path, codex_turn_boundary, decode_codex_line, derive_codex_label,
    CodexTurnBoundary, CODEX_TURN_BOUNDARIES, SOURCE_NAME,
};
use crate::source::fd_probe;
use crate::source::jsonl::{ChildEndUnclaims, JsonlWatcher, ProbeSnapshot, DEFAULT_INITIAL_WINDOW};
use crate::source::{Source, TaggedSender};

/// Codex writes no session-end marker; the reducer's stale-sweep reaps dead
/// sessions. Always false (defer to mtime window + stale-sweep).
fn codex_session_ended(_tail: &[u8]) -> bool {
    false
}

/// Codex's liveness probe: the rollout UUIDs (in `codex_id_from_path`
/// id-space, so they join the watcher's first-sight gate directly) of every
/// live rollout under `sessions_root`, plus the owning pid per id.
///
/// Codex has no session registry (unlike CC's `sessions/<pid>.json`), but a
/// live `codex` process holds its rollout file open in append mode for the
/// whole session (upstream `RolloutRecorder` owns the handle), so a standalone
/// process's open rollout fd is the first-party liveness signal. Codex Desktop
/// multiplexes many completed rollouts through one long-lived app-server pid;
/// for that shape, the latest task/turn boundary must still be a start. Failure
/// is explicit (#223): `None` ONLY when the proc-table
/// enumeration itself fails (the watcher then changes nothing). An ABSENT
/// sessions root is NOT a failure — codex may simply never have run — so it
/// returns `Some(empty)`: a healthy "nothing alive" observation. Per-pid fd
/// failures stay non-failures (a pid exiting mid-probe is normal).
pub fn live_codex_rollout_ids(sessions_root: &Path) -> Option<ProbeSnapshot> {
    live_codex_rollout_ids_with_cache(sessions_root, &Mutex::new(TurnStateCache::default()))
}

fn live_codex_rollout_ids_with_cache(
    sessions_root: &Path,
    turn_states: &Mutex<TurnStateCache>,
) -> Option<ProbeSnapshot> {
    // Canonicalize once per probe call: kernel-reported fd paths are fully
    // resolved (e.g. /tmp → /private/tmp on macOS), so the prefix compare
    // must run against the canonical root or every rollout misses.
    let Ok(root) = sessions_root.canonicalize() else {
        tracing::debug!(
            "codex probe: sessions root {} not canonicalizable; nothing alive there",
            sessions_root.display()
        );
        return Some(ProbeSnapshot::default());
    };
    let pids = fd_probe::pids_by_name("codex")?;
    let pairs = pids.into_iter().flat_map(|pid| {
        fd_probe::open_vnode_paths(pid)
            .into_iter()
            .map(move |path| (pid, path))
    });
    let mut turn_states = turn_states.lock().ok()?;
    Some(rollout_ids_from_paths_with_cache(
        &root,
        pairs,
        &mut turn_states,
    ))
}

/// The pure join half of the probe (unit-testable without FFI): keep the
/// (pid, path) pairs whose path is a `rollout-*.jsonl` under `root`, mapped
/// through `codex_id_from_path` — the watcher's `IdDeriver`, so probe ids and
/// gate ids can't drift. Each surviving pair also binds id → pid for the
/// snapshot's `pid_of` (the exit-watch half).
#[cfg(test)]
fn rollout_ids_from_paths(
    root: &Path,
    pairs: impl Iterator<Item = (i32, PathBuf)>,
) -> ProbeSnapshot {
    rollout_ids_from_paths_with_cache(root, pairs, &mut TurnStateCache::default())
}

fn rollout_ids_from_paths_with_cache(
    root: &Path,
    pairs: impl Iterator<Item = (i32, PathBuf)>,
    turn_states: &mut TurnStateCache,
) -> ProbeSnapshot {
    let mut paths_by_pid: HashMap<i32, Vec<PathBuf>> = HashMap::new();
    for (pid, path) in pairs {
        if path.starts_with(root) && is_rollout_filename(&path) {
            paths_by_pid.entry(pid).or_default().push(path);
        }
    }
    let open_paths: HashSet<_> = paths_by_pid.values().flatten().cloned().collect();

    let mut snap = ProbeSnapshot::default();
    for (pid, paths) in paths_by_pid {
        let distinct_path_count = paths.iter().collect::<HashSet<_>>().len();
        for path in paths {
            // A standalone CLI owns one rollout for its whole lifetime, so an
            // open fd remains authoritative even while that session is idle.
            // Codex desktop's app-server is different: one long-lived process
            // keeps many completed task rollouts open. For that multiplexed
            // shape, only the rollout whose latest turn boundary is a start is
            // live. Otherwise historical tasks fill every desk and starve new
            // hook events before the renderer's idle cap can apply.
            if distinct_path_count > 1 && !turn_states.running_turn(&path).unwrap_or(false) {
                tracing::debug!(
                    "codex probe: pid {pid} multiplexes completed rollout {}; excluding",
                    path.display()
                );
                continue;
            }
            tracing::debug!("codex probe: pid {pid} holds {} open", path.display());
            let id = codex_id_from_path(&path);
            // Two live processes holding ONE rollout open (a resume overlap) must
            // not bind id→pid by proc-enumeration order — the same determinism
            // rule as the CC registry fold's no-startedAt arm (#252): larger pid
            // wins, arbitrary but stable for live processes.
            let bound = snap.pid_of.entry(id).or_insert(pid);
            if pid > *bound {
                *bound = pid;
            }
        }
    }
    turn_states
        .by_path
        .retain(|path, _| open_paths.contains(path));
    snap
}

const TURN_SCAN_BLOCK_BYTES: u64 = 64 * 1024;
const TURN_MARKER_OVERLAP_BYTES: u64 = 64;

#[derive(Clone, Copy)]
struct CachedTurnState {
    file_len: u64,
    running: Option<bool>,
}

#[derive(Default)]
struct TurnStateCache {
    by_path: HashMap<PathBuf, CachedTurnState>,
}

impl TurnStateCache {
    fn running_turn(&mut self, path: &Path) -> Option<bool> {
        let previous = self.by_path.get(path).copied();
        let mut file = std::fs::File::open(path).ok()?;
        let file_len = file.metadata().ok()?.len();
        if previous.is_some_and(|state| state.file_len == file_len) {
            return previous.and_then(|state| state.running);
        }

        let scan_floor = previous
            .filter(|state| file_len > state.file_len)
            .map_or(0, |state| {
                state.file_len.saturating_sub(TURN_MARKER_OVERLAP_BYTES)
            });
        let appended_state = scan_latest_turn_marker(&mut file, file_len, scan_floor);
        let running = appended_state.or_else(|| {
            previous
                .filter(|state| file_len >= state.file_len)
                .and_then(|state| state.running)
        });
        self.by_path
            .insert(path.to_path_buf(), CachedTurnState { file_len, running });
        running
    }
}

#[cfg(test)]
fn rollout_has_running_turn(path: &Path) -> bool {
    TurnStateCache::default()
        .running_turn(path)
        .unwrap_or(false)
}

fn scan_latest_turn_marker(
    file: &mut std::fs::File,
    file_len: u64,
    scan_floor: u64,
) -> Option<bool> {
    let mut newer_edge = file_len;

    while newer_edge > scan_floor {
        let start = newer_edge
            .saturating_sub(TURN_SCAN_BLOCK_BYTES)
            .max(scan_floor);
        let read_end = (newer_edge + TURN_MARKER_OVERLAP_BYTES).min(file_len);
        let mut buf = vec![0; (read_end - start) as usize];
        if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut buf).is_err() {
            return None;
        }
        if let Some(running) = latest_turn_marker(&buf) {
            return Some(running);
        }
        newer_edge = start;
    }
    None
}

fn latest_turn_marker(buf: &[u8]) -> Option<bool> {
    let mut latest = None;
    for (inner, _) in CODEX_TURN_BOUNDARIES {
        for needle in [
            format!(r#""type":"{inner}""#),
            format!(r#""type": "{inner}""#),
        ] {
            let Some(position) = buf
                .windows(needle.len())
                .rposition(|window| window == needle.as_bytes())
            else {
                continue;
            };
            let Some(boundary) = codex_turn_boundary("event_msg", inner) else {
                continue;
            };
            if latest.is_none_or(|(latest_position, _)| position > latest_position) {
                latest = Some((position, boundary));
            }
        }
    }
    latest.map(|(_, boundary)| boundary == CodexTurnBoundary::Started)
}

fn is_rollout_filename(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("jsonl")
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("rollout-"))
}

/// Attach the probe ONLY for codex's first-party layout: the standard
/// `~/.codex/sessions` shape (the root's file_name is literally `sessions`
/// AND its parent's is `.codex`) or the resolved `codex_home()/sessions` for
/// THIS environment (a `CODEX_HOME` user's real rollout root — codex itself
/// writes there, and rejecting it would silently drop the whole liveness
/// ladder for a supported config). Mirrors `cc_sessions_dir`'s gating: a
/// `--codex-sessions-root /tmp/fixture` replay points at an arbitrary dir,
/// and those runs must keep the pure-mtime first-sight gate (the probe is
/// additive-only; a replayed rollout vouched for by a coincidentally-running
/// codex would resurrect as live).
fn codex_probe_root(sessions_root: &Path) -> Option<PathBuf> {
    codex_probe_root_resolved(sessions_root, &codex_home())
}

/// The injectable core of [`codex_probe_root`] (mirrors
/// `platform::resolve_codex_home`'s testable split): `home` is the resolved
/// codex home for this environment.
fn codex_probe_root_resolved(sessions_root: &Path, home: &Path) -> Option<PathBuf> {
    if sessions_root.file_name().and_then(|n| n.to_str()) != Some("sessions") {
        return None;
    }
    let parent = sessions_root.parent();
    let parent_is_codex =
        parent.and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some(".codex");
    // A parent that IS the resolved codex home is first-party even when not
    // named `.codex` — the CODEX_HOME case (`codex_home()` honors the env
    // var the same way `default_paths` does, one resolution for both).
    let parent_is_resolved_home = parent.is_some_and(|p| p == home);
    if !parent_is_codex && !parent_is_resolved_home {
        return None;
    }
    // Not canonicalized here: the dir may not exist yet at wiring time
    // (codex never run); `live_codex_rollout_ids` canonicalizes per probe
    // call, which also picks up a root created after startup.
    Some(sessions_root.to_path_buf())
}

const FIRST_PARTY_INITIAL_WINDOW: Duration = Duration::from_secs(30);

fn codex_startup_window(sessions_root: &Path) -> Duration {
    codex_startup_window_resolved(sessions_root, &codex_home())
}

fn codex_startup_window_resolved(sessions_root: &Path, home: &Path) -> Duration {
    codex_startup_window_resolved_with_probe_support(
        sessions_root,
        home,
        cfg!(any(target_os = "macos", target_os = "linux")),
    )
}

fn codex_startup_window_resolved_with_probe_support(
    sessions_root: &Path,
    home: &Path,
    supports_fd_probe: bool,
) -> Duration {
    if supports_fd_probe && codex_probe_root_resolved(sessions_root, home).is_some() {
        FIRST_PARTY_INITIAL_WINDOW
    } else {
        DEFAULT_INITIAL_WINDOW
    }
}

/// Source that watches the Codex session transcript directory.
pub struct CodexSource {
    pub sessions_root: PathBuf,
    /// The #246 child-end un-claim side-channel — Codex is consumer-only:
    /// its `SubagentStop` hooks ride the shared socket the `HookRouter`
    /// owns (whose tee is the producer), and THIS watcher releases the ended
    /// child's rollout claim so a multi-turn child's turn-N+1 append
    /// re-registers (the motivating #246 case). The runtime shares ONE
    /// handle across the router + the CC and Codex watchers; `None` disables
    /// it (bare test construction).
    pub child_end_unclaims: Option<ChildEndUnclaims>,
}

impl CodexSource {
    pub fn default_paths() -> Self {
        Self {
            sessions_root: codex_home().join("sessions"),
            child_end_unclaims: None,
        }
    }
}

impl Source for CodexSource {
    fn name(&self) -> &str {
        SOURCE_NAME
    }

    async fn run(self: Box<Self>, tx: TaggedSender) -> Result<()> {
        let mut watcher = JsonlWatcher::new(
            self.sessions_root.clone(),
            SOURCE_NAME.to_string(),
            decode_codex_line,
            derive_codex_label,
            codex_session_ended,
        )
        .with_id_deriver(codex_id_from_path)
        .with_initial_window(codex_startup_window(&self.sessions_root));
        if let Some(root) = codex_probe_root(&self.sessions_root) {
            let turn_states = std::sync::Arc::new(Mutex::new(TurnStateCache::default()));
            watcher = watcher.with_liveness_probe(std::sync::Arc::new(move || {
                live_codex_rollout_ids_with_cache(&root, &turn_states)
            }));
        }
        if let Some(unclaims) = &self.child_end_unclaims {
            watcher = watcher.with_child_end_unclaims(unclaims.clone());
        }
        watcher.run(tx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_session_ended_is_always_false() {
        // Codex writes no end marker — the checker always defers to the
        // mtime window + stale-sweep.
        assert!(!codex_session_ended(b"anything"));
        assert!(!codex_session_ended(b""));
    }

    // ---- liveness probe (open-rollout FD binding) ----

    const UUID: &str = "019e7762-9ded-7e33-be41-946ecf105bf4";

    fn snap_of(root: &Path, paths: Vec<PathBuf>) -> ProbeSnapshot {
        rollout_ids_from_paths(root, paths.into_iter().map(|p| (42, p)))
    }

    #[test]
    fn rollout_under_root_yields_its_uuid_bound_to_its_pid() {
        let root = Path::new("/home/u/.codex/sessions");
        // Real layout nests YYYY/MM/DD below the root — starts_with must
        // admit the whole subtree, not only direct children.
        let nested = root.join(format!(
            "2026/06/10/rollout-2026-06-10T08-00-00-{UUID}.jsonl"
        ));
        let got = snap_of(root, vec![nested]);
        assert_eq!(
            got.ids().cloned().collect::<Vec<_>>(),
            vec![UUID.to_string()]
        );
        // #223: the snapshot binds each id to the OWNING pid (the exit-watch
        // half) — the (42, path) pair above must survive the join intact.
        assert_eq!(got.pid_of.get(UUID), Some(&42));
    }

    #[test]
    fn shared_rollout_binds_the_larger_pid_regardless_of_enumeration_order() {
        // Two live processes holding ONE rollout (a resume overlap, #252's
        // codex sibling): the binding must be the deterministic tiebreak
        // winner in BOTH presentation orders, never last-writer-wins.
        let root = Path::new("/home/u/.codex/sessions");
        let path = root.join(format!(
            "2026/06/10/rollout-2026-06-10T08-00-00-{UUID}.jsonl"
        ));
        for pids in [[100, 200], [200, 100]] {
            let got = rollout_ids_from_paths(root, pids.into_iter().map(|p| (p, path.clone())));
            assert_eq!(
                got.ids().cloned().collect::<Vec<_>>(),
                vec![UUID.to_string()]
            );
            assert_eq!(
                got.pid_of.get(UUID),
                Some(&200),
                "the larger pid must win in both enumeration orders"
            );
        }
    }

    #[test]
    fn multiplexed_codex_process_admits_only_rollouts_with_a_running_turn() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let active_id = "019e7762-9ded-7e33-be41-946ecf105bf5";
        let completed_id = "019e7762-9ded-7e33-be41-946ecf105bf6";
        let active = root.join(format!("rollout-2026-06-10T08-00-00-{active_id}.jsonl"));
        let completed = root.join(format!("rollout-2026-06-10T08-00-00-{completed_id}.jsonl"));
        std::fs::write(
            &active,
            concat!(
                r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
                "\n",
                r#"{"type":"event_msg","payload":{"type":"task_started"}}"#,
                "\n"
            ),
        )
        .unwrap();
        std::fs::write(
            &completed,
            concat!(
                r#"{"type":"event_msg","payload":{"type":"task_started"}}"#,
                "\n",
                r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#,
                "\n"
            ),
        )
        .unwrap();

        let got = rollout_ids_from_paths(root, [(42, active), (42, completed)].into_iter());

        assert_eq!(
            got.ids().cloned().collect::<Vec<_>>(),
            vec![active_id.to_string()],
            "a desktop app-server that holds many historical rollouts open must not vouch for completed tasks"
        );
        assert_eq!(got.pid_of.get(active_id), Some(&42));
    }

    #[test]
    fn multiplexed_rollout_keeps_a_running_turn_when_its_start_is_far_from_eof() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join(format!("rollout-2026-06-10T08-00-00-{UUID}.jsonl"));
        let mut body = br#"{"type":"event_msg","payload":{"type":"task_started"}}
"#
        .to_vec();
        body.extend(std::iter::repeat_n(b'x', 17 * 1024 * 1024));
        std::fs::write(&path, body).unwrap();

        assert!(
            rollout_has_running_turn(&path),
            "a long running task must not be declared complete because its start marker is older than a fixed tail window"
        );
    }

    #[test]
    fn cached_turn_state_updates_from_only_new_rollout_bytes() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join(format!("rollout-2026-06-10T08-00-00-{UUID}.jsonl"));
        std::fs::write(
            &path,
            concat!(
                r#"{"type":"event_msg","payload":{"type":"task_started"}}"#,
                "\n"
            ),
        )
        .unwrap();
        let mut states = TurnStateCache::default();
        assert_eq!(states.running_turn(&path), Some(true));

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"event_msg","payload":{{"type":"task_complete"}}}}"#
        )
        .unwrap();
        assert_eq!(states.running_turn(&path), Some(false));
    }

    #[test]
    fn rollout_outside_root_is_excluded() {
        let root = Path::new("/home/u/.codex/sessions");
        let outside = PathBuf::from(format!("/tmp/elsewhere/rollout-1-{UUID}.jsonl"));
        let got = snap_of(root, vec![outside]);
        assert!(got.is_empty());
        assert!(got.pid_of.is_empty());
    }

    #[test]
    fn non_rollout_files_under_root_are_excluded() {
        let root = Path::new("/home/u/.codex/sessions");
        let wrong_stem = root.join("2026/06/10/history.jsonl");
        let wrong_ext = root.join(format!("2026/06/10/rollout-1-{UUID}.log"));
        let no_ext = root.join("2026/06/10/rollout-noext");
        assert!(snap_of(root, vec![wrong_stem, wrong_ext, no_ext]).is_empty());
    }

    #[test]
    fn probe_root_requires_dot_codex_sessions_layout() {
        assert_eq!(
            codex_probe_root(Path::new("/home/u/.codex/sessions")),
            Some(PathBuf::from("/home/u/.codex/sessions"))
        );
        // A fixture replay root must get NO probe (pure-mtime behavior).
        assert_eq!(codex_probe_root(Path::new("/tmp/fixture")), None);
        // A bare relative `sessions` has no parent to check.
        assert_eq!(codex_probe_root(Path::new("sessions")), None);
    }

    #[test]
    fn probe_root_accepts_resolved_codex_home_sessions_layout() {
        // A CODEX_HOME-shaped layout: the resolved home is NOT named
        // `.codex`, but its `sessions` child is codex's first-party rollout
        // root for this environment — the probe must attach, or CODEX_HOME
        // users silently lose the entire liveness ladder (admission bypass,
        // ProofOfLife, negative vouch, instant exit). The env→home
        // resolution itself is pinned by `platform::resolve_codex_home`'s
        // unit tests; this pins the probe gate against the resolved value.
        let home = tempfile::tempdir().unwrap();
        let sessions = home.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        assert_eq!(
            codex_probe_root_resolved(&sessions, home.path()),
            Some(sessions.clone())
        );
        // Replay roots stay probe-less even with a custom home resolved.
        assert_eq!(
            codex_probe_root_resolved(Path::new("/tmp/fixture"), home.path()),
            None
        );
        // `sessions` under a parent that is neither `.codex` nor the
        // resolved home is not first-party.
        assert_eq!(
            codex_probe_root_resolved(Path::new("/srv/other/sessions"), home.path()),
            None
        );
    }

    #[test]
    fn first_party_startup_window_excludes_old_finished_sessions() {
        let home = tempfile::tempdir().unwrap();
        let sessions = home.path().join("sessions");
        assert_eq!(
            codex_startup_window_resolved(&sessions, home.path()),
            std::time::Duration::from_secs(30)
        );
        assert_eq!(
            codex_startup_window_resolved(Path::new("/tmp/fixture"), home.path()),
            std::time::Duration::from_secs(3600),
            "custom replay roots keep the upstream history window"
        );
        assert_eq!(
            codex_startup_window_resolved_with_probe_support(&sessions, home.path(), false),
            DEFAULT_INITIAL_WINDOW,
            "platforms without an fd probe must retain the full startup reconstruction window"
        );
    }

    #[test]
    fn live_ids_for_missing_root_is_some_empty_not_a_failure() {
        // canonicalize() fails on a nonexistent dir, but an ABSENT root is
        // not a probe failure — codex may simply never have run. Some(empty)
        // is the healthy "nothing alive" observation (#223: None would freeze
        // the negative-vouch ledger forever on machines without codex).
        let missing = Path::new("/definitely/not/a/real/.codex/sessions");
        let snap = live_codex_rollout_ids(missing).expect("absent root is not a probe failure");
        assert!(snap.is_empty());
        assert!(snap.pid_of.is_empty());
    }

    #[test]
    fn live_ids_for_unrelated_root_is_empty() {
        // Real FFI smoke: whatever processes exist, none hold a rollout open
        // under a fresh tempdir.
        let dir = tempfile::tempdir().unwrap();
        let snap = live_codex_rollout_ids(dir.path())
            .expect("a healthy system's enumeration must succeed");
        assert!(snap.is_empty());
    }
}
