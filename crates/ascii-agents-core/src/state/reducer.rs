use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use crate::source::AgentEvent;
use crate::state::{ActivityState, AgentSlot, SceneState};
use crate::AgentId;

/// Which transport produced an event — used for dedup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Hook,
    Jsonl,
}

/// Window in which a Hook event suppresses a later Jsonl event with the same tool_use_id.
pub const HOOK_WINS_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
pub struct Reducer {
    /// Track recent hook-derived events so JSONL duplicates can be dropped.
    recent_hook_tool_uses: HashMap<(AgentId, String), SystemTime>,
    /// Per-agent set of Task tool_use_ids currently in flight. CC's hook
    /// payload sets `transcript_path` to the PARENT'S transcript even when a
    /// subagent is the actor, so subagent hook events hash to the parent's
    /// AgentId. While the parent has any Task in flight, hook
    /// ActivityStart/End events for that AgentId are dropped — JSONL has
    /// correct attribution to the subagent's own AgentId.
    active_tasks: HashMap<AgentId, HashSet<String>>,
    /// Monotonic counter for human-readable labels (cc#1, cc#2, ...).
    next_label_n: u32,
}

impl Reducer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        scene: &mut SceneState,
        event: AgentEvent,
        now: SystemTime,
        from: Transport,
    ) {
        self.gc(now);
        let id = event.agent_id();

        // Subagent-leak suppression: if this AgentId currently has any Task
        // tool in flight, hook ActivityStart/End events for it are almost
        // certainly subagent work misattributed to the parent. Drop them and
        // defer to JSONL, which targets the subagent's own AgentId. The
        // Task's own PostToolUse is exempt — its tool_use_id matches one we
        // are tracking, so it passes through and clears the slot.
        if from == Transport::Hook {
            let in_task = self
                .active_tasks
                .get(&id)
                .is_some_and(|s| !s.is_empty());
            let suppress = match &event {
                AgentEvent::ActivityStart { .. } => in_task,
                AgentEvent::ActivityEnd { tool_use_id, .. } => {
                    let is_task_self_end = tool_use_id.as_ref().is_some_and(|t| {
                        self.active_tasks
                            .get(&id)
                            .is_some_and(|s| s.contains(t))
                    });
                    in_task && !is_task_self_end
                }
                _ => false,
            };
            if suppress {
                return;
            }
        }

        // Dedup: drop JSONL events that match a recent Hook event by tool_use_id.
        if from == Transport::Jsonl {
            if let Some(tuid) = event_tool_use_id(&event) {
                if self
                    .recent_hook_tool_uses
                    .contains_key(&(id, tuid.to_string()))
                {
                    return;
                }
            }
        }

        if from == Transport::Hook {
            if let Some(tuid) = event_tool_use_id(&event) {
                self.recent_hook_tool_uses
                    .insert((id, tuid.to_string()), now);
            }
        }

        // Track active Task tool_use_ids from either transport. HashSet is
        // idempotent so duplicate inserts from both hook+jsonl are harmless.
        //
        // Side effect: when the parent gains a Task, also mark it as
        // Active("Delegating") so it doesn't look idle/asleep while its
        // subagents do the visible work. When the last Task drains, the
        // next normal hook/JSONL event will reset its state.
        match &event {
            AgentEvent::ActivityStart {
                agent_id,
                tool_use_id: Some(tuid),
                detail: Some(d),
                ..
            } if is_task_detail(d) => {
                self.active_tasks
                    .entry(*agent_id)
                    .or_default()
                    .insert(tuid.clone());
                if let Some(slot) = scene.agents.get_mut(agent_id) {
                    slot.state = ActivityState::Active {
                        activity: crate::source::Activity::Typing,
                        tool_use_id: Some(tuid.clone()),
                        detail: Some("Delegating".to_string()),
                    };
                    slot.state_started_at = now;
                }
            }
            AgentEvent::ActivityEnd {
                agent_id,
                tool_use_id: Some(tuid),
            } => {
                if let Some(set) = self.active_tasks.get_mut(agent_id) {
                    set.remove(tuid);
                    if set.is_empty() {
                        if let Some(slot) = scene.agents.get_mut(agent_id) {
                            slot.state = ActivityState::Idle;
                            slot.state_started_at = now;
                        }
                    }
                }
            }
            _ => {}
        }

        match event {
            AgentEvent::SessionStart {
                agent_id,
                source,
                session_id,
                cwd,
            } => {
                if scene.agents.contains_key(&agent_id) {
                    return;
                }
                let Some(desk_index) = scene.next_free_desk() else {
                    tracing::warn!(
                        ?agent_id,
                        cwd = %cwd.display(),
                        session_id = %session_id,
                        max_desks = scene.max_desks,
                        "dropped SessionStart — all desks occupied; bump --max-desks"
                    );
                    return;
                };
                self.next_label_n += 1;
                let label = cwd
                    .file_name()
                    .and_then(|n| n.to_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .unwrap_or_else(|| format!("cc#{}", self.next_label_n));
                // Disambiguation for multiple sessions sharing a cwd happens
                // at render time, not here — we don't want to suffix unique
                // sessions with a noisy `·xxxx` they don't need.
                scene.agents.insert(
                    agent_id,
                    AgentSlot {
                        agent_id,
                        source,
                        session_id,
                        cwd,
                        label,
                        state: ActivityState::Idle,
                        state_started_at: now,
                        desk_index,
                    },
                );
            }
            AgentEvent::ActivityStart {
                agent_id,
                activity,
                tool_use_id,
                detail,
            } => {
                if let Some(slot) = scene.agents.get_mut(&agent_id) {
                    slot.state = ActivityState::Active {
                        activity,
                        tool_use_id,
                        detail,
                    };
                    slot.state_started_at = now;
                }
            }
            AgentEvent::ActivityEnd { agent_id, .. } => {
                if let Some(slot) = scene.agents.get_mut(&agent_id) {
                    slot.state = ActivityState::Idle;
                    slot.state_started_at = now;
                }
            }
            AgentEvent::Waiting { agent_id, reason } => {
                if let Some(slot) = scene.agents.get_mut(&agent_id) {
                    slot.state = ActivityState::Waiting { reason };
                    slot.state_started_at = now;
                }
            }
            AgentEvent::Rename { agent_id, label } => {
                if let Some(slot) = scene.agents.get_mut(&agent_id) {
                    if slot.label != label {
                        slot.label = label;
                    }
                }
            }
            AgentEvent::SessionEnd { agent_id } => {
                scene.agents.remove(&agent_id);
                self.active_tasks.remove(&agent_id);
            }
        }
    }

    fn gc(&mut self, now: SystemTime) {
        // SystemTime::duration_since returns Err when `ts` is in the future
        // (clock went backwards). Drop those — stale entries either way.
        self.recent_hook_tool_uses.retain(|_, ts| {
            now.duration_since(*ts)
                .is_ok_and(|d| d < HOOK_WINS_WINDOW)
        });
    }
}

fn event_tool_use_id(ev: &AgentEvent) -> Option<&str> {
    match ev {
        AgentEvent::ActivityStart { tool_use_id, .. }
        | AgentEvent::ActivityEnd { tool_use_id, .. } => tool_use_id.as_deref(),
        _ => None,
    }
}

fn is_task_detail(detail: &str) -> bool {
    // The decoder formats ActivityStart detail as "{tool_name}{target}", so
    // Task tool calls produce "Task" or "Task: ..." (Task currently has no
    // target template, so usually just "Task").
    detail == "Task" || detail.starts_with("Task:") || detail.starts_with("Task ")
}
