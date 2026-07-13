use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use pixtuoid_core::state::{ActivityState, AgentSlot, ToolKind};
use pixtuoid_core::{AgentId, SceneState};

const SOURCE: &str = "visual-coworker";
const RESIDENTS: [(&str, &str); 3] = [
    ("tom", "Tom (Head of IBD)"),
    ("amy", "Amy (Head of IR)"),
    ("jess", "Jess (Head of Strategy)"),
];

/// Builds a render-only scene containing the persistent office residents.
/// The reducer's authoritative scene is never mutated, so these residents
/// cannot enter the dashboard, focus flow, headless output, or agent lifecycle.
pub(crate) struct VisualCoworkers {
    names: BTreeMap<String, String>,
    started_at: SystemTime,
}

impl VisualCoworkers {
    pub(crate) fn new(names: BTreeMap<String, String>) -> Self {
        Self {
            names,
            started_at: SystemTime::now(),
        }
    }

    pub(crate) fn render_scene(&self, scene: &SceneState, now: SystemTime) -> SceneState {
        let mut rendered = scene.clone();
        let vivian = scene
            .agents
            .values()
            .filter(|slot| slot.parent_id.is_none() && slot.label.as_ref() == "Vivian")
            .max_by_key(|slot| (slot.last_event_at, slot.agent_id));
        let active_resident = vivian.and_then(|slot| {
            matches!(slot.state, ActivityState::Active { .. })
                .then_some(slot.tool_call_count.saturating_sub(1) as usize % RESIDENTS.len())
        });
        let cwd = vivian
            .map(|slot| Arc::clone(&slot.cwd))
            .unwrap_or_else(|| Arc::from(Path::new("/")));

        for (index, (key, fallback_name)) in RESIDENTS.iter().enumerate() {
            let Some(desk_index) = rendered.next_free_desk() else {
                break;
            };
            let is_active = active_resident == Some(index);
            let state = if is_active {
                ActivityState::Active {
                    tool_use_id: None,
                    detail: Some(Arc::from("Assisting")),
                    kind: ToolKind::Other,
                }
            } else {
                ActivityState::Idle
            };
            let state_started_at = if is_active {
                vivian
                    .map(|slot| slot.state_started_at)
                    .unwrap_or(self.started_at)
            } else {
                self.started_at
            };
            let agent_id = AgentId::from_parts(SOURCE, key);
            let label = self
                .names
                .get(*key)
                .cloned()
                .unwrap_or_else(|| (*fallback_name).to_string());

            rendered.agents.insert(
                agent_id,
                AgentSlot {
                    agent_id,
                    source: Arc::from(SOURCE),
                    session_id: Arc::from(*key),
                    cwd: Arc::clone(&cwd),
                    label: label.into(),
                    state,
                    state_started_at,
                    last_event_at: now,
                    created_at: self.started_at,
                    exiting_at: None,
                    pending_idle_at: None,
                    desk_index,
                    floor_idx: rendered.floor_of(desk_index),
                    tool_call_count: 0,
                    active_ms: 0,
                    unknown_cwd: false,
                    parent_id: None,
                    pid: None,
                    model: None,
                    effort: None,
                },
            );
        }

        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixtuoid_core::state::{GlobalDeskIndex, MAX_FLOORS};

    fn slot(
        id: AgentId,
        label: &str,
        desk: usize,
        parent_id: Option<AgentId>,
        state: ActivityState,
        tool_call_count: u32,
    ) -> AgentSlot {
        AgentSlot {
            agent_id: id,
            source: Arc::from("codex"),
            session_id: Arc::from(label),
            cwd: Arc::from(Path::new("/tmp/secondbrain-os")),
            label: label.to_string().into(),
            state,
            state_started_at: SystemTime::UNIX_EPOCH,
            last_event_at: SystemTime::UNIX_EPOCH,
            created_at: SystemTime::UNIX_EPOCH,
            exiting_at: None,
            pending_idle_at: None,
            desk_index: GlobalDeskIndex(desk),
            floor_idx: 0,
            tool_call_count,
            active_ms: 0,
            unknown_cwd: false,
            parent_id,
            pid: None,
            model: None,
            effort: None,
        }
    }

    fn scene_with_vivian(state: ActivityState, tool_call_count: u32) -> SceneState {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 4;
        let mut scene = SceneState::new(capacities);
        let id = AgentId::from_parts("codex", "root");
        scene
            .agents
            .insert(id, slot(id, "Vivian", 0, None, state, tool_call_count));
        scene
    }

    #[test]
    fn active_vivian_rotates_exactly_one_active_resident() {
        let coworkers = VisualCoworkers::new(BTreeMap::new());
        for (tool_call_count, expected) in [
            (1, "Tom (Head of IBD)"),
            (2, "Amy (Head of IR)"),
            (3, "Jess (Head of Strategy)"),
            (4, "Tom (Head of IBD)"),
        ] {
            let scene = scene_with_vivian(
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                tool_call_count,
            );
            let rendered = coworkers.render_scene(&scene, SystemTime::UNIX_EPOCH);
            let active: Vec<_> = rendered
                .agents
                .values()
                .filter(|slot| slot.source.as_ref() == SOURCE)
                .filter(|slot| matches!(slot.state, ActivityState::Active { .. }))
                .map(|slot| slot.label.as_ref())
                .collect();
            assert_eq!(active, [expected]);
            assert_eq!(
                scene.agents.len(),
                1,
                "the real scene must remain unchanged"
            );
        }
    }

    #[test]
    fn idle_vivian_keeps_all_residents_ambient() {
        let scene = scene_with_vivian(ActivityState::Idle, 9);
        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        let residents: Vec<_> = rendered
            .agents
            .values()
            .filter(|slot| slot.source.as_ref() == SOURCE)
            .collect();
        assert_eq!(residents.len(), 3);
        assert!(residents
            .iter()
            .all(|slot| matches!(slot.state, ActivityState::Idle)));
    }

    #[test]
    fn real_agents_take_desk_capacity_before_residents() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 2;
        let mut scene = SceneState::new(capacities);
        let root = AgentId::from_parts("codex", "root");
        let child = AgentId::from_parts("codex", "child");
        scene
            .agents
            .insert(root, slot(root, "Vivian", 0, None, ActivityState::Idle, 0));
        scene.agents.insert(
            child,
            slot(
                child,
                "Analyst 01",
                1,
                Some(root),
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                1,
            ),
        );

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        assert_eq!(rendered.agents.len(), 2);
        assert!(rendered
            .agents
            .values()
            .all(|slot| slot.source.as_ref() != SOURCE));
    }
}
