use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use pixtuoid_core::state::{ActivityState, AgentSlot};
use pixtuoid_core::{AgentId, SceneState};

use super::TASK_RESIDENTS;

const SOURCE: &str = "visual-coworker";
const MAX_VISIBLE_IDLE_AGENTS: usize = 8;
const QUIET_COWORKERS: [(&str, &str); 3] = [
    ("alex", "Alex"),
    ("tristan-pembroke", "Tristan Pembroke"),
    ("maya", "Maya"),
];

fn resident_rows() -> impl Iterator<Item = (&'static str, &'static str)> {
    TASK_RESIDENTS
        .iter()
        .map(|resident| (resident.key, resident.fallback_name))
        .chain(QUIET_COWORKERS)
}

#[derive(Clone, Copy)]
enum IdleDeparture {
    Departing {
        started_at: SystemTime,
        generation: SystemTime,
    },
    Hidden {
        generation: SystemTime,
        historical: bool,
    },
}

fn counts_toward_idle_cap(slot: &AgentSlot) -> bool {
    matches!(slot.state, ActivityState::Idle) && slot.exiting_at.is_none()
}

/// Builds a render-only scene containing the persistent office residents.
/// The reducer's authoritative scene is never mutated, so these residents
/// cannot enter the dashboard, focus flow, headless output, or agent lifecycle.
pub(crate) struct VisualCoworkers {
    names: BTreeMap<String, String>,
    started_at: SystemTime,
    departing_idle: HashMap<AgentId, IdleDeparture>,
}

impl VisualCoworkers {
    pub(crate) fn new(names: BTreeMap<String, String>) -> Self {
        Self {
            names,
            started_at: SystemTime::now(),
            departing_idle: HashMap::new(),
        }
    }

    pub(crate) fn render_scene(&mut self, scene: &SceneState, now: SystemTime) -> SceneState {
        let mut rendered = scene.clone();
        let vivian = scene
            .agents
            .values()
            .filter(|slot| slot.parent_id.is_none() && slot.label.as_ref() == "Vivian")
            .max_by_key(|slot| (slot.last_event_at, slot.agent_id));
        let cwd = vivian
            .map(|slot| Arc::clone(&slot.cwd))
            .unwrap_or_else(|| Arc::from(Path::new("/")));
        let baseline_labels: HashSet<String> = std::iter::once("Vivian".to_string())
            .chain(resident_rows().map(|(key, fallback_name)| {
                self.names
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| fallback_name.to_string())
            }))
            .collect();

        // Transcript history can keep streaming after the first frame and can
        // replay old Active states. Only a hook proves this exact slot lifetime
        // is live, so historical slots never displace the quiet cast.
        let suppressed_history: Vec<_> = rendered
            .agents
            .iter()
            .filter(|(id, slot)| {
                slot.label.as_ref() != "Vivian" && !scene.is_live_generation(**id, slot.created_at)
            })
            .map(|(id, slot)| (*id, slot.created_at))
            .collect();
        for (id, generation) in suppressed_history {
            rendered.agents.remove(&id);
            self.departing_idle.insert(
                id,
                IdleDeparture::Hidden {
                    generation,
                    historical: true,
                },
            );
        }

        // Completed departures stay hidden while the authoritative session is
        // still idle. Removing them before resident insertion frees their desk
        // in the render-only clone without deleting real session history.
        self.departing_idle
            .retain(|id, departure| match (scene.agents.get(id), departure) {
                (
                    Some(slot),
                    IdleDeparture::Hidden {
                        generation,
                        historical,
                    },
                ) => {
                    slot.created_at == *generation
                        && slot.label.as_ref() != "Vivian"
                        && (!*historical || !scene.is_live_generation(*id, *generation))
                        && !matches!(
                            slot.state,
                            ActivityState::Active { .. } | ActivityState::Waiting { .. }
                        )
                }
                (Some(slot), IdleDeparture::Departing { generation, .. }) => {
                    slot.created_at == *generation && counts_toward_idle_cap(slot)
                }
                (None, _) => false,
            });
        for (id, departure) in &self.departing_idle {
            if matches!(departure, IdleDeparture::Hidden { .. }) {
                rendered.agents.remove(id);
            }
        }

        // The default view opens on floor zero. A long transcript history can
        // leave the newest idle sessions on upper floors, so keep the single
        // visual Vivian anchored in the main office without mutating the real
        // scene. Swap desks with the ground-floor occupant to preserve unique
        // desk assignments in the render-only clone.
        if let Some(vivian_slot) = vivian.filter(|slot| slot.floor_idx != 0) {
            if let Some(target_desk) = rendered.floor_range(0).next() {
                let target_desk = pixtuoid_core::state::GlobalDeskIndex(target_desk);
                let displaced = rendered
                    .agents
                    .iter()
                    .find(|(_, slot)| slot.desk_index == target_desk)
                    .map(|(id, _)| *id);
                if let Some(displaced_id) = displaced {
                    if let Some(slot) = rendered.agents.get_mut(&displaced_id) {
                        slot.desk_index = vivian_slot.desk_index;
                        slot.floor_idx = vivian_slot.floor_idx;
                    }
                }
                if let Some(slot) = rendered.agents.get_mut(&vivian_slot.agent_id) {
                    slot.desk_index = target_desk;
                    slot.floor_idx = 0;
                }
            }
        }

        // Avatar components rotate once per Pocket Office launch, not when an
        // older Codex session happened to begin. This changes only the visual
        // clone and keeps the selection stable for every frame of this run.
        if let Some(vivian_slot) = vivian {
            if let Some(slot) = rendered.agents.get_mut(&vivian_slot.agent_id) {
                slot.created_at = self.started_at;
            }
        }

        if vivian.is_none() {
            if let Some(desk_index) = rendered.next_free_desk() {
                let agent_id = AgentId::from_parts(SOURCE, "vivian");
                rendered.agents.insert(
                    agent_id,
                    AgentSlot {
                        agent_id,
                        source: Arc::from(SOURCE),
                        session_id: Arc::from("vivian"),
                        cwd: Arc::clone(&cwd),
                        label: "Vivian".into(),
                        state: ActivityState::Idle,
                        state_started_at: self.started_at,
                        last_event_at: self.started_at,
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
        }

        // A full desk map must still settle to the eight-person quiet cast.
        // Reserve enough seats by walking generic idle history out first.
        let baseline_present = baseline_labels
            .iter()
            .filter(|label| {
                rendered
                    .agents
                    .values()
                    .any(|slot| slot.label.as_ref() == label.as_str())
            })
            .count();
        let free_desks = rendered
            .total_capacity()
            .saturating_sub(rendered.agents.len());
        let seats_to_reserve = MAX_VISIBLE_IDLE_AGENTS
            .saturating_sub(baseline_present)
            .saturating_sub(free_desks);
        let mut reserve_candidates: Vec<_> = rendered
            .agents
            .iter()
            .filter(|(_, slot)| {
                counts_toward_idle_cap(slot) && !baseline_labels.contains(slot.label.as_ref())
            })
            .map(|(id, slot)| (*id, slot.last_event_at))
            .collect();
        reserve_candidates.sort_by_key(|(id, last_event_at)| (*last_event_at, *id));
        let forced_excess: HashSet<_> = reserve_candidates
            .into_iter()
            .take(seats_to_reserve)
            .map(|(id, _)| id)
            .collect();

        for (key, fallback_name) in resident_rows() {
            let label = self
                .names
                .get(key)
                .cloned()
                .unwrap_or_else(|| fallback_name.to_string());
            let resident_is_working = rendered
                .agents
                .values()
                .any(|slot| slot.parent_id.is_some() && slot.label.as_ref() == label);
            if resident_is_working {
                continue;
            }
            let Some(desk_index) = rendered.next_free_desk() else {
                break;
            };
            let state = ActivityState::Idle;
            let state_started_at = self.started_at;
            let agent_id = AgentId::from_parts(SOURCE, key);

            rendered.agents.insert(
                agent_id,
                AgentSlot {
                    agent_id,
                    source: Arc::from(SOURCE),
                    session_id: Arc::from(key),
                    cwd: Arc::clone(&cwd),
                    label: label.into(),
                    state,
                    state_started_at,
                    last_event_at: self.started_at,
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

        let mut idle_ids: Vec<_> = rendered
            .agents
            .iter()
            .filter(|(_, slot)| counts_toward_idle_cap(slot))
            .map(|(id, _)| *id)
            .collect();
        idle_ids.sort_by_key(|id| {
            let slot = &rendered.agents[id];
            (
                baseline_labels.contains(slot.label.as_ref()),
                slot.label.as_ref() == "Vivian",
                slot.floor_idx == 0,
                slot.source.as_ref() != SOURCE,
                slot.last_event_at,
                slot.agent_id.raw(),
            )
        });
        let visible_idle: HashSet<_> = idle_ids
            .into_iter()
            .rev()
            .take(MAX_VISIBLE_IDLE_AGENTS)
            .collect();
        let mut excess_idle: HashSet<_> = rendered
            .agents
            .iter()
            .filter(|(id, slot)| counts_toward_idle_cap(slot) && !visible_idle.contains(id))
            .map(|(id, _)| *id)
            .collect();
        excess_idle.extend(forced_excess);
        self.departing_idle.retain(|id, departure| {
            matches!(departure, IdleDeparture::Hidden { .. }) || excess_idle.contains(id)
        });

        for id in excess_idle {
            let Some(generation) = rendered.agents.get(&id).map(|slot| slot.created_at) else {
                continue;
            };
            let departure = self
                .departing_idle
                .entry(id)
                .or_insert(IdleDeparture::Departing {
                    started_at: now,
                    generation,
                });
            match *departure {
                IdleDeparture::Departing {
                    started_at,
                    generation,
                } => {
                    let exit_finished = now.duration_since(started_at).is_ok_and(|elapsed| {
                        elapsed > pixtuoid_core::state::reducer::EXIT_GRACE_WINDOW
                    });
                    if exit_finished {
                        *departure = IdleDeparture::Hidden {
                            generation,
                            historical: false,
                        };
                        rendered.agents.remove(&id);
                    } else if let Some(slot) = rendered.agents.get_mut(&id) {
                        slot.exiting_at = Some(started_at);
                    }
                }
                IdleDeparture::Hidden { .. } => {
                    rendered.agents.remove(&id);
                }
            }
        }

        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixtuoid_core::source::{AgentEvent, Transport};
    use pixtuoid_core::state::reducer::Reducer;
    use pixtuoid_core::state::{GlobalDeskIndex, ToolKind, MAX_FLOORS};
    use pixtuoid_scene::layout::SceneLayout;
    use pixtuoid_scene::pose::{
        derive_state_only, est_wander_cycle_ms, seated_dwell_ms, takes_trip, Pose,
    };
    use std::time::Duration;

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

    fn mark_all_live(scene: &mut SceneState) {
        let identities: Vec<_> = scene
            .agents
            .values()
            .map(|slot| {
                (
                    slot.agent_id,
                    slot.source.to_string(),
                    slot.session_id.to_string(),
                    slot.cwd.to_path_buf(),
                )
            })
            .collect();
        let mut reducer = Reducer::new();
        for (agent_id, source, session_id, cwd) in identities {
            reducer.apply(
                scene,
                AgentEvent::Identity {
                    agent_id,
                    source,
                    session_id,
                    cwd: Some(cwd),
                    pid: None,
                },
                SystemTime::UNIX_EPOCH,
                Transport::Hook,
            );
        }
    }

    fn scene_with_vivian(state: ActivityState, tool_call_count: u32) -> SceneState {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 8;
        let mut scene = SceneState::new(capacities);
        let id = AgentId::from_parts("codex", "root");
        scene
            .agents
            .insert(id, slot(id, "Vivian", 0, None, state, tool_call_count));
        scene
    }

    #[test]
    fn empty_scene_renders_the_eight_person_office_baseline() {
        let scene = SceneState::new([8; MAX_FLOORS]);
        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);

        assert_eq!(rendered.agents.len(), 8);
        assert_eq!(
            rendered
                .agents
                .values()
                .filter(|slot| slot.label.as_ref() == "Vivian")
                .count(),
            1,
            "the empty office must seed exactly one visual Vivian"
        );
        assert!(rendered
            .agents
            .values()
            .all(|slot| matches!(slot.state, ActivityState::Idle)));
        for label in [
            "Tom (Head of IBD)",
            "Amy (Head of IR)",
            "Jess (Head of Strategy)",
            "Alison",
            "Alex",
            "Tristan Pembroke",
            "Maya",
        ] {
            assert!(
                rendered
                    .agents
                    .values()
                    .any(|slot| slot.label.as_ref() == label),
                "quiet office is missing {label}"
            );
        }
        assert!(
            scene.agents.is_empty(),
            "rendering must not mutate real sessions"
        );
    }

    #[test]
    fn real_vivian_uses_one_stable_pocket_office_launch_timestamp() {
        let scene = scene_with_vivian(ActivityState::Idle, 0);
        let original_created_at = scene
            .agents
            .values()
            .find(|slot| slot.label.as_ref() == "Vivian")
            .expect("real Vivian")
            .created_at;
        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        let launch_timestamp = coworkers.started_at;

        let first = coworkers.render_scene(&scene, SystemTime::UNIX_EPOCH);
        let later =
            coworkers.render_scene(&scene, SystemTime::UNIX_EPOCH + Duration::from_secs(60));
        let rendered_timestamp = |rendered: &SceneState| {
            rendered
                .agents
                .values()
                .find(|slot| slot.label.as_ref() == "Vivian")
                .expect("rendered Vivian")
                .created_at
        };

        assert_ne!(original_created_at, launch_timestamp);
        assert_eq!(rendered_timestamp(&first), launch_timestamp);
        assert_eq!(rendered_timestamp(&later), launch_timestamp);
        assert_eq!(
            scene
                .agents
                .values()
                .find(|slot| slot.label.as_ref() == "Vivian")
                .expect("real Vivian")
                .created_at,
            original_created_at,
            "render-only launch styling must not mutate the real session"
        );
    }

    #[test]
    fn persistent_idle_resident_reaches_walking_pose_after_desk_dwell() {
        let layout = SceneLayout::compute(192, 160, Some(7)).expect("office layout");
        assert!(
            !layout.waypoints.is_empty(),
            "test layout needs a destination"
        );

        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        let started_at = coworkers.started_at;
        let (key, cycle_n) = resident_rows()
            .find_map(|(key, _)| {
                let agent_id = AgentId::from_parts(SOURCE, key);
                (0..32)
                    .find(|&cycle_n| takes_trip(agent_id, cycle_n))
                    .map(|cycle_n| (key, cycle_n))
            })
            .expect("at least one resident takes a deterministic trip");
        let agent_id = AgentId::from_parts(SOURCE, key);
        let elapsed_ms = cycle_n * est_wander_cycle_ms(agent_id) + seated_dwell_ms(agent_id) + 1;
        let now = started_at
            .checked_add(Duration::from_millis(elapsed_ms))
            .expect("test timestamp");
        let scene = SceneState::new([7; MAX_FLOORS]);
        let rendered = coworkers.render_scene(&scene, now);
        let resident = rendered.agents.get(&agent_id).expect("visual resident");

        assert!(
            matches!(
                derive_state_only(resident, now, &layout),
                Some(Pose::Walking { .. })
            ),
            "an idle visual resident must enter its scheduled wander trip"
        );
    }

    #[test]
    fn restart_keeps_eight_visible_on_the_ground_floor() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 8;
        capacities[1] = 8;
        let mut scene = SceneState::new(capacities);

        for desk in 0..7 {
            let id = AgentId::from_parts("codex", &format!("ground-{desk}"));
            let mut ground = slot(
                id,
                &format!("Ground {desk}"),
                desk,
                None,
                ActivityState::Idle,
                0,
            );
            ground.last_event_at = SystemTime::UNIX_EPOCH + Duration::from_secs(desk as u64);
            scene.agents.insert(id, ground);
        }

        let vivian = AgentId::from_parts("codex", "vivian-upstairs");
        let mut upstairs_vivian = slot(vivian, "Vivian", 8, None, ActivityState::Idle, 0);
        upstairs_vivian.floor_idx = 1;
        upstairs_vivian.last_event_at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        scene.agents.insert(vivian, upstairs_vivian);

        for offset in 1..7 {
            let id = AgentId::from_parts("codex", &format!("upstairs-{offset}"));
            let mut upstairs = slot(
                id,
                &format!("Upstairs {offset}"),
                8 + offset,
                Some(vivian),
                ActivityState::Idle,
                0,
            );
            upstairs.floor_idx = 1;
            upstairs.last_event_at =
                SystemTime::UNIX_EPOCH + Duration::from_secs(100 + offset as u64);
            scene.agents.insert(id, upstairs);
        }

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        let ground = pixtuoid_scene::floor::project_floor_scene(&rendered, 0);

        assert_eq!(
            ground
                .agents
                .values()
                .filter(|slot| slot.exiting_at.is_none())
                .count(),
            8,
            "the default floor must never restart empty while idle sessions sit upstairs"
        );
        assert_eq!(
            ground
                .agents
                .values()
                .filter(|slot| slot.label.as_ref() == "Vivian")
                .count(),
            1
        );
    }

    #[test]
    fn active_vivian_does_not_fake_resident_activity() {
        let scene = scene_with_vivian(
            ActivityState::Active {
                tool_use_id: None,
                detail: None,
                kind: ToolKind::Other,
            },
            4,
        );
        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        assert!(rendered
            .agents
            .values()
            .filter(|slot| slot.source.as_ref() == SOURCE)
            .all(|slot| matches!(slot.state, ActivityState::Idle)));
        assert_eq!(
            scene.agents.len(),
            1,
            "the real scene must remain unchanged"
        );
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
        assert_eq!(residents.len(), 7);
        assert!(residents
            .iter()
            .all(|slot| matches!(slot.state, ActivityState::Idle)));
    }

    #[test]
    fn real_resident_task_suppresses_the_duplicate_idle_resident() {
        let mut scene = scene_with_vivian(ActivityState::Idle, 0);
        let vivian = scene
            .agents
            .values()
            .find(|slot| slot.label.as_ref() == "Vivian")
            .expect("Vivian")
            .agent_id;
        let tom = AgentId::from_parts("codex", "real-tom");
        scene.agents.insert(
            tom,
            slot(
                tom,
                "Tom (Head of IBD)",
                1,
                Some(vivian),
                ActivityState::Active {
                    tool_use_id: None,
                    detail: Some(Arc::from("Working")),
                    kind: ToolKind::Other,
                },
                1,
            ),
        );
        mark_all_live(&mut scene);

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        let toms: Vec<_> = rendered
            .agents
            .values()
            .filter(|slot| slot.label.as_ref() == "Tom (Head of IBD)")
            .collect();

        assert_eq!(toms.len(), 1);
        assert_eq!(toms[0].agent_id, tom);
        assert_ne!(toms[0].source.as_ref(), SOURCE);

        scene.agents.get_mut(&tom).expect("real Tom").exiting_at = Some(SystemTime::UNIX_EPOCH);
        let departing =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        assert_eq!(
            departing
                .agents
                .values()
                .filter(|slot| slot.label.as_ref() == "Tom (Head of IBD)")
                .count(),
            1,
            "the idle resident must wait until the real task finishes its departure"
        );
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
                "Alex",
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
        mark_all_live(&mut scene);

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        assert_eq!(rendered.agents.len(), 2);
        assert!(rendered
            .agents
            .values()
            .all(|slot| slot.source.as_ref() != SOURCE));
    }

    #[test]
    fn render_scene_caps_idle_agents_at_eight_without_hiding_work() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 16;
        let mut scene = SceneState::new(capacities);
        let vivian = AgentId::from_parts("codex", "root");
        scene.agents.insert(
            vivian,
            slot(vivian, "Vivian", 0, None, ActivityState::Idle, 0),
        );

        for desk in 2..=8 {
            let id = AgentId::from_parts("codex", &format!("idle-{desk}"));
            let mut idle = slot(
                id,
                &format!("Idle {desk}"),
                desk,
                Some(vivian),
                ActivityState::Idle,
                0,
            );
            idle.last_event_at = SystemTime::UNIX_EPOCH + Duration::from_secs(desk as u64);
            scene.agents.insert(id, idle);
        }

        let active = AgentId::from_parts("codex", "active");
        scene.agents.insert(
            active,
            slot(
                active,
                "Active",
                9,
                Some(vivian),
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                1,
            ),
        );
        let waiting = AgentId::from_parts("codex", "waiting");
        scene.agents.insert(
            waiting,
            slot(
                waiting,
                "Waiting",
                10,
                Some(vivian),
                ActivityState::Waiting {
                    reason: Arc::from("permission"),
                },
                0,
            ),
        );
        let real_agent_count = scene.agents.len();
        mark_all_live(&mut scene);

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        let visible_idle = rendered
            .agents
            .values()
            .filter(|slot| counts_toward_idle_cap(slot))
            .count();

        assert_eq!(visible_idle, 8);
        assert!(rendered.agents.contains_key(&active));
        assert!(rendered.agents.contains_key(&waiting));
        assert!(rendered.agents.contains_key(&vivian));
        assert_eq!(
            scene.agents.len(),
            real_agent_count,
            "rendering must not mutate real sessions"
        );
    }

    #[test]
    fn idle_cap_never_caps_thirty_working_agents() {
        let mut scene = SceneState::new([16; MAX_FLOORS]);
        let mut active_ids = Vec::new();
        for desk in 0..30 {
            let id = AgentId::from_parts("codex", &format!("working-{desk}"));
            let mut working = slot(
                id,
                &format!("Working {desk}"),
                desk,
                None,
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                1,
            );
            working.floor_idx = scene.floor_of(working.desk_index);
            scene.agents.insert(id, working);
            active_ids.push(id);
        }
        mark_all_live(&mut scene);

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);

        assert_eq!(
            rendered
                .agents
                .values()
                .filter(|slot| matches!(slot.state, ActivityState::Active { .. }))
                .count(),
            30
        );
        assert!(active_ids.iter().all(|id| rendered.agents.contains_key(id)));
    }

    #[test]
    fn quiet_office_replaces_old_idle_sessions_with_the_persistent_cast() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 8;
        let mut scene = SceneState::new(capacities);
        let vivian = AgentId::from_parts("codex", "root");
        scene.agents.insert(
            vivian,
            slot(vivian, "Vivian", 0, None, ActivityState::Idle, 0),
        );
        for desk in 1..=7 {
            let id = AgentId::from_parts("codex", &format!("real-{desk}"));
            scene.agents.insert(
                id,
                slot(
                    id,
                    &format!("Real {desk}"),
                    desk,
                    Some(vivian),
                    ActivityState::Idle,
                    0,
                ),
            );
        }

        let settled =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);
        let labels: HashSet<_> = settled
            .agents
            .values()
            .map(|slot| slot.label.as_ref())
            .collect();

        assert_eq!(settled.agents.len(), 8);
        assert_eq!(
            labels,
            HashSet::from([
                "Vivian",
                "Tom (Head of IBD)",
                "Amy (Head of IR)",
                "Jess (Head of Strategy)",
                "Alison",
                "Alex",
                "Tristan Pembroke",
                "Maya",
            ])
        );
    }

    #[test]
    fn late_startup_history_never_replaces_the_quiet_cast() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 8;
        let empty = SceneState::new(capacities);
        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        let first = coworkers.render_scene(&empty, SystemTime::UNIX_EPOCH);
        let first_labels: HashSet<_> = first
            .agents
            .values()
            .map(|slot| slot.label.to_string())
            .collect();

        let mut replay = SceneState::new(capacities);
        let vivian = AgentId::from_parts("codex", "late-root");
        replay.agents.insert(
            vivian,
            slot(vivian, "Vivian", 0, None, ActivityState::Idle, 0),
        );
        let stale_tom = AgentId::from_parts("codex", "late-tom");
        replay.agents.insert(
            stale_tom,
            slot(
                stale_tom,
                "Tom (Head of IBD)",
                1,
                Some(vivian),
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                1,
            ),
        );
        for desk in 2..=7 {
            let id = AgentId::from_parts("codex", &format!("late-idle-{desk}"));
            replay.agents.insert(
                id,
                slot(
                    id,
                    &format!("Late Idle {desk}"),
                    desk,
                    Some(vivian),
                    ActivityState::Idle,
                    0,
                ),
            );
        }

        let replaying =
            coworkers.render_scene(&replay, coworkers.started_at + Duration::from_secs(10));
        assert_eq!(
            replaying
                .agents
                .values()
                .map(|slot| slot.label.to_string())
                .collect::<HashSet<_>>(),
            first_labels
        );
        assert!(replaying
            .agents
            .values()
            .filter(|slot| slot.label.as_ref() != "Vivian")
            .all(|slot| slot.source.as_ref() == SOURCE));

        replay.agents.get_mut(&stale_tom).unwrap().state = ActivityState::Idle;
        let late = coworkers.render_scene(&replay, coworkers.started_at + Duration::from_secs(12));
        let late_labels: HashSet<_> = late
            .agents
            .values()
            .map(|slot| slot.label.to_string())
            .collect();

        assert_eq!(late_labels, first_labels);
        assert_eq!(late.agents.len(), 8);
        assert!(late.agents.values().all(|slot| slot.exiting_at.is_none()));
        assert!(late
            .agents
            .values()
            .filter(|slot| slot.label.as_ref() != "Vivian")
            .all(|slot| slot.source.as_ref() == SOURCE));
    }

    #[test]
    fn startup_hidden_session_stays_hidden_when_the_reducer_marks_it_exiting() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 8;
        let empty = SceneState::new(capacities);
        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        coworkers.render_scene(&empty, SystemTime::UNIX_EPOCH);

        let mut replay = SceneState::new(capacities);
        let stale = AgentId::from_parts("codex", "late-exit");
        replay.agents.insert(
            stale,
            slot(stale, "Late Exit", 0, None, ActivityState::Idle, 0),
        );
        replay.agents.get_mut(&stale).unwrap().exiting_at = Some(SystemTime::UNIX_EPOCH);
        assert!(!coworkers
            .render_scene(&replay, coworkers.started_at + Duration::from_secs(10))
            .agents
            .contains_key(&stale));
    }

    #[test]
    fn live_hook_proof_clears_a_historical_hidden_latch_even_while_idle() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 8;
        let mut scene = SceneState::new(capacities);
        let id = AgentId::from_parts("codex", "idle-proof");
        scene
            .agents
            .insert(id, slot(id, "Idle Proof", 0, None, ActivityState::Idle, 0));
        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        assert!(!coworkers
            .render_scene(&scene, SystemTime::UNIX_EPOCH)
            .agents
            .contains_key(&id));

        mark_all_live(&mut scene);

        assert!(coworkers
            .render_scene(&scene, SystemTime::UNIX_EPOCH + Duration::from_secs(1))
            .agents
            .contains_key(&id));
    }

    #[test]
    fn transcript_only_source_activity_is_live_without_a_hook() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 16;
        let mut scene = SceneState::new(capacities);
        let id = AgentId::from_parts("copilot", "jsonl-only");
        let started = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let mut reducer = Reducer::new();
        reducer.apply(
            &mut scene,
            AgentEvent::SessionStart {
                agent_id: id,
                source: "copilot".into(),
                session_id: "jsonl-only".into(),
                cwd: Path::new("/tmp/secondbrain-os").to_path_buf(),
                parent_id: None,
            },
            started,
            Transport::Jsonl,
        );
        reducer.apply(
            &mut scene,
            AgentEvent::ActivityStart {
                agent_id: id,
                tool_use_id: None,
                detail: None,
            },
            started + Duration::from_millis(1),
            Transport::Jsonl,
        );

        assert!(VisualCoworkers::new(BTreeMap::new())
            .render_scene(&scene, started + Duration::from_secs(10))
            .agents
            .contains_key(&id));
    }

    #[test]
    fn reused_agent_id_does_not_inherit_a_prior_lifetimes_working_observation() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 16;
        let mut scene = SceneState::new(capacities);
        let id = AgentId::from_parts("codex", "reused-agent");
        let first_started = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let mut reducer = Reducer::new();
        reducer.apply(
            &mut scene,
            AgentEvent::SessionStart {
                agent_id: id,
                source: "codex".into(),
                session_id: "reused-agent".into(),
                cwd: Path::new("/tmp/secondbrain-os").to_path_buf(),
                parent_id: None,
            },
            first_started,
            Transport::Hook,
        );
        reducer.apply(
            &mut scene,
            AgentEvent::ActivityStart {
                agent_id: id,
                tool_use_id: None,
                detail: None,
            },
            first_started + Duration::from_millis(1),
            Transport::Hook,
        );
        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        assert!(coworkers
            .render_scene(&scene, SystemTime::UNIX_EPOCH)
            .agents
            .contains_key(&id));

        let mut second_life = slot(id, "Second Life", 0, None, ActivityState::Idle, 0);
        second_life.created_at = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        scene.agents.insert(id, second_life);

        assert!(!coworkers
            .render_scene(&scene, SystemTime::UNIX_EPOCH + Duration::from_millis(50))
            .agents
            .contains_key(&id));
    }

    #[test]
    fn idle_cap_does_not_hide_an_agent_who_is_already_walking_out() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 16;
        let mut scene = SceneState::new(capacities);
        let vivian = AgentId::from_parts("codex", "root");
        scene.agents.insert(
            vivian,
            slot(vivian, "Vivian", 0, None, ActivityState::Idle, 0),
        );
        for desk in 1..=8 {
            let id = AgentId::from_parts("codex", &format!("idle-{desk}"));
            let mut idle = slot(
                id,
                &format!("Idle {desk}"),
                desk,
                Some(vivian),
                ActivityState::Idle,
                0,
            );
            idle.last_event_at = SystemTime::UNIX_EPOCH + Duration::from_secs(desk as u64);
            scene.agents.insert(id, idle);
        }
        let exiting = AgentId::from_parts("codex", "exiting");
        let mut departure = slot(exiting, "Leaving", 9, Some(vivian), ActivityState::Idle, 0);
        departure.exiting_at = Some(SystemTime::UNIX_EPOCH);
        scene.agents.insert(exiting, departure);
        mark_all_live(&mut scene);

        let rendered =
            VisualCoworkers::new(BTreeMap::new()).render_scene(&scene, SystemTime::UNIX_EPOCH);

        assert!(
            rendered.agents.contains_key(&exiting),
            "an existing walk-out must finish even when the idle office is full"
        );
    }

    #[test]
    fn newly_excess_idle_agent_walks_out_before_becoming_hidden() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 16;
        let mut scene = SceneState::new(capacities);
        let vivian = AgentId::from_parts("codex", "root");
        scene.agents.insert(
            vivian,
            slot(vivian, "Vivian", 0, None, ActivityState::Idle, 0),
        );
        for desk in 1..=7 {
            let id = AgentId::from_parts("codex", &format!("idle-{desk}"));
            let mut idle = slot(
                id,
                &format!("Idle {desk}"),
                desk,
                Some(vivian),
                ActivityState::Idle,
                0,
            );
            idle.last_event_at = SystemTime::UNIX_EPOCH + Duration::from_secs(desk as u64);
            scene.agents.insert(id, idle);
        }
        let overflow = AgentId::from_parts("codex", "overflow");
        scene.agents.insert(
            overflow,
            slot(
                overflow,
                "Overflow",
                7,
                Some(vivian),
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                1,
            ),
        );
        mark_all_live(&mut scene);

        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        let active_frame = coworkers.render_scene(&scene, SystemTime::UNIX_EPOCH);
        assert!(active_frame.agents.contains_key(&overflow));

        let exit_started = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        scene.agents.get_mut(&overflow).unwrap().state = ActivityState::Idle;
        let departure_frame = coworkers.render_scene(&scene, exit_started);
        let departing = departure_frame
            .agents
            .get(&overflow)
            .expect("an excess idle agent must remain rendered while walking to the door");
        assert_eq!(departing.exiting_at, Some(exit_started));

        let grace = pixtuoid_core::state::reducer::EXIT_GRACE_WINDOW;
        let almost_finished = exit_started + grace - Duration::from_millis(1);
        let final_departure_frame = coworkers.render_scene(&scene, almost_finished);
        assert_eq!(
            final_departure_frame.agents[&overflow].exiting_at,
            Some(exit_started),
            "the departure timestamp must remain stable for the full walk-out window"
        );

        let after_departure = exit_started + grace + Duration::from_millis(1);
        let hidden_frame = coworkers.render_scene(&scene, after_departure);
        assert!(!hidden_frame.agents.contains_key(&overflow));
    }

    #[test]
    fn completed_idle_departure_stays_hidden_after_clock_regression() {
        let mut capacities = [0; MAX_FLOORS];
        capacities[0] = 16;
        let mut scene = SceneState::new(capacities);
        let vivian = AgentId::from_parts("codex", "root");
        scene.agents.insert(
            vivian,
            slot(vivian, "Vivian", 0, None, ActivityState::Idle, 0),
        );
        for desk in 1..=8 {
            let id = AgentId::from_parts("codex", &format!("idle-{desk}"));
            let mut idle = slot(
                id,
                &format!("Idle {desk}"),
                desk,
                Some(vivian),
                ActivityState::Idle,
                0,
            );
            idle.last_event_at = SystemTime::UNIX_EPOCH + Duration::from_secs(desk as u64);
            scene.agents.insert(id, idle);
        }

        let overflow = AgentId::from_parts("codex", "idle-1");
        scene.agents.insert(
            overflow,
            slot(
                overflow,
                "Idle 1",
                1,
                Some(vivian),
                ActivityState::Active {
                    tool_use_id: None,
                    detail: None,
                    kind: ToolKind::Other,
                },
                1,
            ),
        );
        mark_all_live(&mut scene);
        let mut coworkers = VisualCoworkers::new(BTreeMap::new());
        coworkers.render_scene(&scene, SystemTime::UNIX_EPOCH);
        let exit_started = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        scene.agents.get_mut(&overflow).unwrap().state = ActivityState::Idle;
        let departure_frame = coworkers.render_scene(&scene, exit_started);
        assert_eq!(
            departure_frame.agents[&overflow].exiting_at,
            Some(exit_started)
        );

        let after_departure = exit_started
            + pixtuoid_core::state::reducer::EXIT_GRACE_WINDOW
            + Duration::from_millis(1);
        assert!(!coworkers
            .render_scene(&scene, after_departure)
            .agents
            .contains_key(&overflow));

        let regressed_clock = exit_started + Duration::from_millis(1);
        assert!(
            !coworkers
                .render_scene(&scene, regressed_clock)
                .agents
                .contains_key(&overflow),
            "a completed departure must stay hidden until the agent becomes visible again"
        );
    }
}
