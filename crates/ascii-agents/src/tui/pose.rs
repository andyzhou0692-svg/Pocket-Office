//! State → pose derivation for the coworking-lounge renderer.
//!
//! Pure function: given an `AgentSlot`, current `SystemTime`, and `Layout`,
//! returns which `Pose` the agent should appear in this frame. Includes the
//! wander state machine for Idle agents (cycles between desk and waypoints).
//!
//! Variation knobs:
//!  * `cycle_ms_for(agent_id)` — per-agent wander cycle length so the office
//!    stops feeling clockwork-synchronized. Range 7..13s.
//!  * Waypoint choice XORs `agent_id` with the current cycle number, so each
//!    cycle the same agent picks a (likely) different waypoint.

use std::time::{Duration, SystemTime};

use ascii_agents_core::state::{ActivityState, AgentSlot};
use ascii_agents_core::walkable::OccupancyOverlay;
use ascii_agents_core::AgentId;

use crate::tui::layout::{Layout, Point, WaypointKind};
use crate::tui::pathfind::Router;

/// Base cycle length. Each agent's actual cycle = base + per-agent jitter.
pub const WANDER_CYCLE_BASE_MS: u64 = 7_000;
/// Maximum extra time added per agent — jitter range is `[0, RANGE)`.
pub const WANDER_CYCLE_RANGE_MS: u64 = 6_000;
/// Phase fractions of a cycle (×1000 to stay in integer math).
const PHASE_SEATED_FRAC: u64 = 389; // 0..389/1000
const PHASE_WALK_OUT_FRAC: u64 = 556; // 389..556/1000
const PHASE_AT_WAYPOINT_FRAC: u64 = 833; // 556..833/1000
                                         // walk-back is 833..1000/1000.

/// Frame-cycle period for animated poses.
pub const TYPING_FRAME_MS: u64 = 140;
pub const WALKING_FRAME_MS: u64 = 220;
pub const TYPING_FRAMES: usize = 2;
pub const WALKING_FRAMES: usize = 2;

/// Deterministic wander-cycle length for one agent. Each agent picks a
/// different speed so walkers don't move in lockstep.
pub fn cycle_ms_for(agent_id: AgentId) -> u64 {
    WANDER_CYCLE_BASE_MS + (agent_id.raw() >> 16) % WANDER_CYCLE_RANGE_MS
}

/// Per-agent wander personality derived from the agent's id hash.
/// Controls how often the agent leaves their desk and whether they prefer
/// aimless wandering vs heading to a named lounge waypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Personality {
    /// Probability (in percent) that this agent takes a trip on any given
    /// cycle. Range: 10..=50. Restless agents wander more.
    pub trip_chance_pct: u8,
    /// Probability (in percent) that a trip is aimless wander vs heading to
    /// a lounge waypoint. Range: 0..=70.
    pub aimless_pref_pct: u8,
}

pub fn personality_for(agent_id: AgentId) -> Personality {
    let h = agent_id.raw();
    Personality {
        trip_chance_pct: (10 + (h % 41)) as u8,  // 10..=50
        aimless_pref_pct: ((h >> 8) % 71) as u8, // 0..=70
    }
}

/// Deterministic per-(agent, cycle) decision: does this agent take a
/// wander trip on this cycle, or stay seated? Trip frequency is driven by
/// per-agent Personality so different agents wander at different rates.
pub fn takes_trip(agent_id: AgentId, cycle_n: u64) -> bool {
    let p = personality_for(agent_id);
    let mix = agent_id.raw() ^ cycle_n.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (mix % 100) < p.trip_chance_pct as u64
}

/// Per-(agent, cycle) decision: when the agent takes a trip, is it an
/// aimless wander (random walkway point) or a directed visit to a named
/// waypoint? Used by `idle_pose` AND by the snapshot example to find
/// agent_ids whose cycle deterministically lands at a target waypoint.
pub fn is_aimless_cycle(agent_id: AgentId, cycle_n: u64) -> bool {
    let p = personality_for(agent_id);
    let type_mix = agent_id.raw() ^ cycle_n.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    (type_mix % 100) < p.aimless_pref_pct as u64
}

/// Per-(agent, cycle) waypoint index. Only meaningful when `takes_trip` is
/// true AND `is_aimless_cycle` is false. Returns 0 if `num_waypoints` is 0.
pub fn waypoint_index_for_cycle(agent_id: AgentId, cycle_n: u64, num_waypoints: usize) -> usize {
    if num_waypoints == 0 {
        return 0;
    }
    ((agent_id.raw() ^ cycle_n) as usize) % num_waypoints
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pose {
    SeatedIdle,
    SeatedTyping {
        frame: usize,
    },
    StandingAtDesk,
    /// At a lounge waypoint. Concrete render depends on the kind:
    ///   Couch    → sit on couch sprite
    ///   Coffee   → standing + holding-coffee sprite
    ///   Others   → plain standing
    AtWaypoint {
        wp: usize,
        kind: WaypointKind,
    },
    Walking {
        from: Point,
        to: Point,
        t_x1000: u16,
        frame: usize,
    },
    /// Standing at a random walkway point (not at any waypoint). The dest field
    /// is the buf-pixel target the agent walked to. Used by aimless wander.
    AimlessAt {
        dest: Point,
    },
}

/// Milliseconds of one-shot walk-from-door entry animation. Overrides the
/// normal state→pose mapping for any newly-spawned agent so SessionStart
/// reads as "someone just walked into the office".
pub const ENTRY_ANIMATION_MS: u64 = 4000;

/// Returns `None` if the slot's desk_index is out of range for `layout`.
///
/// Priority chain (first match wins, walks down):
///   1. **Exit override** — slot has `exiting_at` set → Walking to door.
///      Once the exit window passes, returns `None` (slot will be GC'd).
///   2. **Entry override** — `now - created_at < ENTRY_ANIMATION_MS` →
///      Walking from door to desk. Plays for the first 4 s of the slot's
///      life regardless of state changes.
///   3. **State-driven pose** — Active → SeatedTyping, Waiting →
///      StandingAtDesk, Idle → idle_pose (which itself decides between
///      SeatedIdle / Walking / AtWaypoint / AimlessAt based on the
///      wander state machine).
pub fn derive(slot: &AgentSlot, now: SystemTime, layout: &Layout) -> Option<Pose> {
    let desk = *layout.home_desks.get(slot.desk_index)?;

    // Exit takes priority — once SessionEnd fires we always walk to the
    // door regardless of entry-window or normal state. Use door_threshold
    // (on-floor point below the door) as the walk target so the character
    // doesn't paint through the wall trim.
    if let (Some(exit_time), Some(target)) = (slot.exiting_at, layout.door_threshold) {
        let since_exit = now
            .duration_since(exit_time)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;
        if since_exit < ENTRY_ANIMATION_MS {
            let t = (since_exit * 1000 / ENTRY_ANIMATION_MS).min(1000) as u16;
            let frame = ((since_exit / WALKING_FRAME_MS) as usize) % WALKING_FRAMES;
            return Some(Pose::Walking {
                from: desk,
                to: target,
                t_x1000: t,
                frame,
            });
        }
        // Past exit window: nothing to render, slot will be GC'd shortly.
        return None;
    }

    // Entry animation overrides everything for the first ENTRY_ANIMATION_MS
    // after creation — agent walks in from the door threshold to their desk.
    if let Some(from) = layout.door_threshold {
        let since_spawn = now
            .duration_since(slot.created_at)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;
        if since_spawn < ENTRY_ANIMATION_MS {
            let t = (since_spawn * 1000 / ENTRY_ANIMATION_MS).min(1000) as u16;
            let frame = ((since_spawn / WALKING_FRAME_MS) as usize) % WALKING_FRAMES;
            return Some(Pose::Walking {
                from,
                to: desk,
                t_x1000: t,
                frame,
            });
        }
    }

    let elapsed = now
        .duration_since(slot.state_started_at)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64;

    match &slot.state {
        ActivityState::Active { .. } => {
            let frame = ((elapsed / TYPING_FRAME_MS) as usize) % TYPING_FRAMES;
            Some(Pose::SeatedTyping { frame })
        }
        ActivityState::Waiting { .. } => Some(Pose::StandingAtDesk),
        ActivityState::Idle => Some(idle_pose(slot, desk, layout, elapsed)),
    }
}

/// Per-agent rendered position cache. Updated each frame by
/// `derive_with_routing`, consulted on state transitions so an agent
/// who was mid-walk when their state flipped can complete the walk
/// visually instead of teleporting back to their desk.
#[derive(Debug, Default, Clone)]
pub struct PoseHistory {
    last: std::collections::HashMap<AgentId, (Point, SystemTime)>,
}

impl PoseHistory {
    pub fn new() -> Self {
        Self::default()
    }
    /// Record where an agent was visually placed this frame.
    pub fn record(&mut self, agent_id: AgentId, anchor: Point, now: SystemTime) {
        self.last.insert(agent_id, (anchor, now));
    }
    /// Latest recorded position if it's at most `max_age_ms` old.
    pub fn recent(&self, agent_id: AgentId, max_age_ms: u64, now: SystemTime) -> Option<Point> {
        let (pt, when) = self.last.get(&agent_id).copied()?;
        let age = now.duration_since(when).ok()?.as_millis() as u64;
        if age <= max_age_ms {
            Some(pt)
        } else {
            None
        }
    }
}

/// Duration of the snap-back walk used when state-driven pose would
/// instantly place the agent back at their desk. 600ms is short enough
/// to feel responsive (the user wants to see the tool fire) but long
/// enough to read as motion, not a pop.
const SNAP_BACK_MS: u64 = 900;
/// Minimum manhattan distance (px) from current rendered position to
/// the desk before we bother animating the snap-back. Below this the
/// teleport is invisible and animating wastes a frame.
const SNAP_BACK_MIN_DIST: i32 = 8;

/// Routed variant of `derive`. For Walking poses, asks `router` for an
/// A*-routed polyline (composed against the layout's static mask + the
/// per-frame `overlay`) and converts the global t (0..1000) into a
/// per-segment Walking pose so the character traces the path
/// corner-by-corner instead of cutting through obstacles or other agents.
///
/// `history` is consulted on state transitions: if the agent's pose
/// flipped from a wander walk (or from AtWaypoint) to a desk-bound
/// pose (SeatedTyping / SeatedIdle / StandingAtDesk), we override the
/// instant teleport with a brief walk from the recorded previous
/// position to the desk.
pub fn derive_with_routing(
    slot: &AgentSlot,
    now: SystemTime,
    layout: &Layout,
    router: &mut dyn Router,
    overlay: &OccupancyOverlay,
    history: &mut PoseHistory,
) -> Option<Pose> {
    let raw = derive(slot, now, layout)?;
    // Snap-back override: state-driven poses (SeatedTyping etc.) at the
    // desk would teleport the agent if they were mid-wander when state
    // changed. Replace them with a Walking pose from the previous
    // rendered position over SNAP_BACK_MS.
    let desk_pose = matches!(
        raw,
        Pose::SeatedIdle | Pose::SeatedTyping { .. } | Pose::StandingAtDesk
    );
    let since_state = now
        .duration_since(slot.state_started_at)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64;
    let pose = if desk_pose && since_state < SNAP_BACK_MS {
        if let Some(prev) = history.recent(slot.agent_id, 300, now) {
            let desk = *layout.home_desks.get(slot.desk_index)?;
            let dist =
                (prev.x as i32 - desk.x as i32).abs() + (prev.y as i32 - desk.y as i32).abs();
            if dist >= SNAP_BACK_MIN_DIST {
                // Walk-end target is offset (+6, +4) from the desk pixel so
                // walking_anchor(target) lands on the SAME sprite anchor
                // that seated_anchor(desk) would. Without this offset the
                // sprite jumps ~6 px right + 4 px down at the moment the
                // pose flips from Walking → SeatedTyping. The agent ends
                // visually AT the desk (anchor-equivalent), so there's no
                // perceivable transition flash.
                let snap_target = Point {
                    x: desk.x + 6,
                    y: desk.y + 4,
                };
                let t = (since_state * 1000 / SNAP_BACK_MS).min(1000) as u16;
                let frame = ((since_state / WALKING_FRAME_MS) as usize) % WALKING_FRAMES;
                Pose::Walking {
                    from: prev,
                    to: snap_target,
                    t_x1000: t,
                    frame,
                }
            } else {
                raw
            }
        } else {
            raw
        }
    } else {
        raw
    };

    let Pose::Walking {
        from,
        to,
        t_x1000,
        frame,
    } = pose
    else {
        // Record AtWaypoint / AimlessAt positions too — they're a valid
        // "previous position" for a subsequent snap-back walk.
        let pt = match &pose {
            Pose::AtWaypoint { wp, .. } => layout.waypoints.get(*wp).map(|w| w.pos),
            Pose::AimlessAt { dest } => Some(*dest),
            _ => None,
        };
        if let Some(p) = pt {
            history.record(slot.agent_id, p, now);
        }
        return Some(pose);
    };
    // Per-agent path personality: perturb the routing destination by a
    // few pixels hashed from the agent_id. Different agents heading
    // between the same two waypoints get different cache keys and (in
    // most cases) visibly different polylines — breaks the "ant trail"
    // effect when multiple agents converge on the same place. The last
    // polyline point is then restored to the true `to` so the walker
    // ends at the canonical destination, not the jittered approximation.
    let h = slot.agent_id.raw();
    let jx = ((h % 9) as i32 - 4) as i16;
    let jy = (((h >> 16) % 9) as i32 - 4) as i16;
    let to_jittered = Point {
        x: to.x.saturating_add_signed(jx),
        y: to.y.saturating_add_signed(jy),
    };
    let mut path = router.route(&layout.walkable, overlay, from, to_jittered);
    if let Some(last) = path.last_mut() {
        *last = to;
    }
    if path.len() <= 2 {
        // Straight-line walk — record the interpolated position for
        // next frame's snap-back lookup.
        history.record(slot.agent_id, walking_position(from, to, t_x1000), now);
        return Some(pose);
    }
    // Map global t to a (segment_idx, t_within_segment) using cumulative
    // octile distance — same metric A* used to plan the path, so timing
    // stays uniform along diagonals.
    let mut leg_lens: Vec<u32> = Vec::with_capacity(path.len() - 1);
    for w in path.windows(2) {
        leg_lens.push(octile_distance(w[0], w[1]));
    }
    let total: u32 = leg_lens.iter().sum();
    if total == 0 {
        return Some(pose);
    }
    let traveled = (t_x1000 as u32 * total) / 1000;
    let mut acc: u32 = 0;
    for (i, &leg) in leg_lens.iter().enumerate() {
        if acc + leg >= traveled {
            let into_leg = traveled - acc;
            let seg_t = (into_leg * 1000)
                .checked_div(leg)
                .map(|t| t.min(1000) as u16)
                .unwrap_or(1000);
            // Record the walker's current position for the next frame's
            // snap-back lookup.
            let cur_pos = walking_position(path[i], path[i + 1], seg_t);
            history.record(slot.agent_id, cur_pos, now);
            return Some(Pose::Walking {
                from: path[i],
                to: path[i + 1],
                t_x1000: seg_t,
                frame,
            });
        }
        acc += leg;
    }
    // Past the last segment — snap to final.
    let last = path.len() - 1;
    history.record(slot.agent_id, path[last], now);
    Some(Pose::Walking {
        from: path[last - 1],
        to: path[last],
        t_x1000: 1000,
        frame,
    })
}

/// Pure linear interpolation along the segment from `from` to `to`. The
/// rendering side has its own `walking_position` in renderer.rs that
/// also applies vertical breathing; this one is for history-tracking
/// only (we want the deterministic position, not the breath offset).
fn walking_position(from: Point, to: Point, t_x1000: u16) -> Point {
    let t = t_x1000 as i32;
    let x = (from.x as i32 + (to.x as i32 - from.x as i32) * t / 1000).max(0) as u16;
    let y = (from.y as i32 + (to.y as i32 - from.y as i32) * t / 1000).max(0) as u16;
    Point { x, y }
}

fn octile_distance(a: Point, b: Point) -> u32 {
    let dx = (a.x as i32 - b.x as i32).unsigned_abs();
    let dy = (a.y as i32 - b.y as i32).unsigned_abs();
    14 * dx.min(dy) + 10 * (dx.max(dy) - dx.min(dy))
}

fn idle_pose(slot: &AgentSlot, desk: Point, layout: &Layout, elapsed_ms: u64) -> Pose {
    let cycle_ms = cycle_ms_for(slot.agent_id);
    let cycle_n = elapsed_ms / cycle_ms;
    let phase_t = elapsed_ms % cycle_ms;

    if !takes_trip(slot.agent_id, cycle_n) || layout.waypoints.is_empty() {
        return Pose::SeatedIdle;
    }

    // Per-cycle "trip type" roll. Personality.aimless_pref_pct shifts the
    // mix between lounge waypoint and aimless wander.
    let aimless = is_aimless_cycle(slot.agent_id, cycle_n);

    // Phase boundaries.
    let seated_end = cycle_ms * PHASE_SEATED_FRAC / 1000;
    let walk_out_end = cycle_ms * PHASE_WALK_OUT_FRAC / 1000;
    let at_wp_end = cycle_ms * PHASE_AT_WAYPOINT_FRAC / 1000;

    // Destination: lounge waypoint OR aimless point.
    let (dest, at_dest_pose): (Point, Pose) = if aimless {
        // Aimless: ANY walkable pixel in the office is fair game —
        // meeting room interior, pantry, cubicle aisles, the corridor,
        // anywhere a character could stand. Rejection-sample up to
        // 16 hashed (x, y) pairs against the walkability mask; pick
        // the first walkable one. Falls back to the corridor center
        // if every probe lands on furniture (vanishingly rare).
        //
        // Sampling bounds avoid the top wall band (y < top_margin) and
        // the baseboard (last 3 rows). Deterministic per (agent, cycle).
        let seed = slot.agent_id.raw() ^ cycle_n.wrapping_mul(0xd1b5_4a32_d192_ed03);
        let y_lo = layout.top_margin;
        let y_hi = layout.buf_h.saturating_sub(3);
        let y_range = y_hi.saturating_sub(y_lo).max(1);
        let mut found: Option<Point> = None;
        for i in 0..16u64 {
            let h = seed
                .wrapping_add(i.wrapping_mul(0x9e37_79b9_7f4a_7c15))
                .wrapping_mul(0xc6a4_a793_5bd1_e995);
            let x = (h as u16) % layout.buf_w.max(1);
            let y = y_lo + ((h >> 16) as u16) % y_range;
            if layout.is_walkable(x, y) {
                found = Some(Point { x, y });
                break;
            }
        }
        let p = found.unwrap_or_else(|| {
            // Vanishingly rare fallback — the corridor center is always
            // walkable by construction.
            let c = layout.corridor.unwrap_or(layout.walkway);
            Point {
                x: c.x + c.width / 2,
                y: c.y + c.height / 2,
            }
        });
        (p, Pose::AimlessAt { dest: p })
    } else {
        let wp_idx = waypoint_index_for_cycle(slot.agent_id, cycle_n, layout.waypoints.len());
        let wp = layout.waypoints[wp_idx];
        (
            wp.pos,
            Pose::AtWaypoint {
                wp: wp_idx,
                kind: wp.kind,
            },
        )
    };

    if phase_t < seated_end {
        Pose::SeatedIdle
    } else if phase_t < walk_out_end {
        let span = walk_out_end - seated_end;
        let t = ((phase_t - seated_end) * 1000 / span) as u16;
        let frame = ((elapsed_ms / WALKING_FRAME_MS) as usize) % WALKING_FRAMES;
        Pose::Walking {
            from: desk,
            to: dest,
            t_x1000: t,
            frame,
        }
    } else if phase_t < at_wp_end {
        at_dest_pose
    } else {
        let span = cycle_ms - at_wp_end;
        let t = ((phase_t - at_wp_end) * 1000 / span) as u16;
        let frame = ((elapsed_ms / WALKING_FRAME_MS) as usize) % WALKING_FRAMES;
        Pose::Walking {
            from: dest,
            to: desk,
            t_x1000: t,
            frame,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ascii_agents_core::source::Activity;
    use std::path::PathBuf;
    use std::time::Duration;

    fn slot(state: ActivityState, age_ms: u64) -> (AgentSlot, SystemTime) {
        let id = AgentId::from_transcript_path("/p/a.jsonl");
        let started = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let now = started + Duration::from_millis(age_ms);
        // created_at well before `started` so the entry-animation override
        // doesn't fire in tests that probe regular state→pose mapping.
        let created = started - Duration::from_secs(60);
        let s = AgentSlot {
            agent_id: id,
            source: std::sync::Arc::from("claude-code"),
            session_id: std::sync::Arc::from("abc"),
            cwd: std::sync::Arc::from(PathBuf::from("/repo").as_path()),
            label: std::sync::Arc::from("cc"),
            state,
            state_started_at: started,
            created_at: created,
            exiting_at: None,
            pending_idle_at: None,
            desk_index: 0,
        };
        (s, now)
    }

    fn layout() -> Layout {
        Layout::compute(120, 96, 4).expect("fits")
    }

    fn typing() -> ActivityState {
        ActivityState::Active {
            activity: Activity::Typing,
            tool_use_id: Some("t".into()),
            detail: Some("Edit".into()),
        }
    }

    /// Phase boundary helper using the agent's actual cycle length.
    fn phases(agent_id: AgentId) -> (u64, u64, u64, u64) {
        let c = cycle_ms_for(agent_id);
        (
            c * PHASE_SEATED_FRAC / 1000,
            c * PHASE_WALK_OUT_FRAC / 1000,
            c * PHASE_AT_WAYPOINT_FRAC / 1000,
            c,
        )
    }

    /// Find the lowest cycle index where the agent decides to take a trip.
    /// Pose tests that probe walking/waypoint phases need a known trip cycle
    /// to drive the elapsed offset off of.
    fn first_trip_cycle(agent_id: AgentId) -> u64 {
        (0u64..1000)
            .find(|n| takes_trip(agent_id, *n))
            .expect("agent should trip within first 1000 cycles")
    }

    #[test]
    fn active_state_is_seated_typing_with_cycling_frame() {
        let (s, now) = slot(typing(), 0);
        let l = layout();
        assert_eq!(derive(&s, now, &l), Some(Pose::SeatedTyping { frame: 0 }));
        let (s, now) = slot(typing(), TYPING_FRAME_MS);
        assert_eq!(derive(&s, now, &l), Some(Pose::SeatedTyping { frame: 1 }));
        let (s, now) = slot(typing(), TYPING_FRAME_MS * 2);
        assert_eq!(derive(&s, now, &l), Some(Pose::SeatedTyping { frame: 0 }));
    }

    #[test]
    fn waiting_state_is_standing_at_desk() {
        let (s, now) = slot(
            ActivityState::Waiting {
                reason: "perm".into(),
            },
            5_000,
        );
        let l = layout();
        assert_eq!(derive(&s, now, &l), Some(Pose::StandingAtDesk));
    }

    #[test]
    fn idle_phase_0_is_seated_idle() {
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let (seated_end, _, _, _) = phases(test_slot.agent_id);
        let (s, now) = slot(ActivityState::Idle, seated_end - 1);
        let l = layout();
        assert_eq!(derive(&s, now, &l), Some(Pose::SeatedIdle));
    }

    #[test]
    fn idle_phase_1_is_walking_out() {
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let (seated_end, walk_out_end, _, _) = phases(test_slot.agent_id);
        let cycle = cycle_ms_for(test_slot.agent_id);
        let trip_n = first_trip_cycle(test_slot.agent_id);
        let midpoint = trip_n * cycle + seated_end + (walk_out_end - seated_end) / 2;
        let (s, now) = slot(ActivityState::Idle, midpoint);
        let l = layout();
        match derive(&s, now, &l).expect("pose") {
            Pose::Walking { t_x1000, frame, .. } => {
                assert!((400..=600).contains(&t_x1000), "t_x1000={t_x1000}");
                assert!(frame < WALKING_FRAMES);
            }
            other => panic!("expected Walking, got {other:?}"),
        }
    }

    #[test]
    fn idle_phase_2_is_at_waypoint() {
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let (_, walk_out_end, at_wp_end, _) = phases(test_slot.agent_id);
        let cycle = cycle_ms_for(test_slot.agent_id);
        let trip_n = first_trip_cycle(test_slot.agent_id);
        let midpoint = trip_n * cycle + walk_out_end + (at_wp_end - walk_out_end) / 2;
        let (s, now) = slot(ActivityState::Idle, midpoint);
        let l = layout();
        // Trip cycles land at either a named waypoint or an aimless point,
        // depending on per-agent personality.
        match derive(&s, now, &l).expect("pose") {
            Pose::AtWaypoint { wp, .. } => assert!(wp < l.waypoints.len()),
            Pose::AimlessAt { .. } => {}
            other => panic!("expected AtWaypoint or AimlessAt, got {other:?}"),
        }
    }

    #[test]
    fn idle_phase_3_is_walking_back() {
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let (_, _, at_wp_end, cycle) = phases(test_slot.agent_id);
        let trip_n = first_trip_cycle(test_slot.agent_id);
        let midpoint = trip_n * cycle + at_wp_end + (cycle - at_wp_end) / 2;
        let (s, now) = slot(ActivityState::Idle, midpoint);
        let l = layout();
        match derive(&s, now, &l).expect("pose") {
            Pose::Walking { t_x1000, .. } => {
                assert!((400..=600).contains(&t_x1000));
            }
            other => panic!("expected Walking, got {other:?}"),
        }
    }

    #[test]
    fn takes_trip_fires_roughly_30_percent_of_cycles() {
        let id = AgentId::from_transcript_path("/p/sample.jsonl");
        let trips = (0u64..1000).filter(|n| takes_trip(id, *n)).count();
        // Per-agent trip chance varies 10..=50%, so across 1000 cycles the
        // count is bounded by those extremes with reasonable tolerance.
        assert!(
            (50..=550).contains(&trips),
            "expected 50..=550 trips out of 1000 (personality-driven), got {trips}"
        );
    }

    #[test]
    fn personality_varies_across_agents() {
        let ps: Vec<Personality> = (0..20)
            .map(|i| personality_for(AgentId::from_transcript_path(&format!("/p/{i}.jsonl"))))
            .collect();
        let trip_chances: std::collections::HashSet<u8> =
            ps.iter().map(|p| p.trip_chance_pct).collect();
        assert!(
            trip_chances.len() >= 5,
            "expected variance in trip_chance_pct"
        );
        for p in &ps {
            assert!((10..=50).contains(&p.trip_chance_pct));
            assert!(p.aimless_pref_pct <= 70);
        }
    }

    #[test]
    fn non_trip_cycle_is_seated_idle_throughout() {
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let id = test_slot.agent_id;
        let cycle = cycle_ms_for(id);
        // Find a cycle where the agent does NOT trip.
        let stay_n = (0u64..100)
            .find(|n| !takes_trip(id, *n))
            .expect("agent should have a non-trip cycle");
        // Sample 10 points across that cycle; all should be SeatedIdle.
        for k in 0..10 {
            let t = stay_n * cycle + (k * cycle / 10);
            let (s, now) = slot(ActivityState::Idle, t);
            let l = layout();
            assert_eq!(
                derive(&s, now, &l),
                Some(Pose::SeatedIdle),
                "t={t} should be SeatedIdle on non-trip cycle"
            );
        }
    }

    #[test]
    fn idle_cycle_loops_after_one_cycle() {
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let cycle = cycle_ms_for(test_slot.agent_id);
        let (s_early, now_early) = slot(ActivityState::Idle, 1_000);
        let (s_loop, now_loop) = slot(ActivityState::Idle, 1_000 + cycle);
        let l = layout();
        // Same phase within cycle, BUT waypoint differs because cycle_n changed.
        // Only the destination changes — the phase itself is the same kind.
        let e = derive(&s_early, now_early, &l).expect("e");
        let lp = derive(&s_loop, now_loop, &l).expect("loop");
        assert!(
            matches!((e, lp), (Pose::SeatedIdle, Pose::SeatedIdle)),
            "1s into any cycle should be SeatedIdle. got early={e:?} loop={lp:?}"
        );
    }

    #[test]
    fn entry_animation_overrides_normal_pose_for_first_4s() {
        let id = AgentId::from_transcript_path("/p/entry.jsonl");
        let now0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        // created_at == now0, so since_spawn = 1500ms at probe time
        let s = AgentSlot {
            agent_id: id,
            source: std::sync::Arc::from("claude-code"),
            session_id: std::sync::Arc::from("abc"),
            cwd: std::sync::Arc::from(PathBuf::from("/repo").as_path()),
            label: std::sync::Arc::from("cc"),
            state: ActivityState::Idle,
            state_started_at: now0,
            created_at: now0,
            exiting_at: None,
            pending_idle_at: None,
            desk_index: 0,
        };
        let probe = now0 + Duration::from_millis(1500);
        let l = layout();
        match derive(&s, probe, &l).expect("pose") {
            Pose::Walking { t_x1000, .. } => {
                // 1500/4000 = 0.375 → t_x1000 ~= 375
                assert!((300..=450).contains(&t_x1000), "t_x1000={t_x1000}");
            }
            other => panic!("expected Walking entry, got {other:?}"),
        }
    }

    #[test]
    fn derive_returns_none_when_desk_index_out_of_range() {
        let (mut s, now) = slot(ActivityState::Idle, 0);
        s.desk_index = 999;
        assert!(derive(&s, now, &layout()).is_none());
    }

    #[test]
    fn cycle_ms_for_varies_across_agents() {
        // Sanity: a handful of different agent ids should not all map to the
        // same cycle length.
        let ids: Vec<AgentId> = (0..10)
            .map(|i| AgentId::from_transcript_path(&format!("/p/{i}.jsonl")))
            .collect();
        let cycles: std::collections::HashSet<u64> =
            ids.iter().map(|id| cycle_ms_for(*id)).collect();
        assert!(
            cycles.len() >= 3,
            "expected multiple distinct cycle lengths, got {cycles:?}"
        );
        for c in &cycles {
            assert!(
                *c >= WANDER_CYCLE_BASE_MS && *c < WANDER_CYCLE_BASE_MS + WANDER_CYCLE_RANGE_MS
            );
        }
    }

    #[test]
    fn waypoint_choice_changes_across_cycles_for_same_agent() {
        let l = layout();
        let (test_slot, _) = slot(ActivityState::Idle, 0);
        let cycle = cycle_ms_for(test_slot.agent_id);
        let (_, walk_out_end, at_wp_end, _) = phases(test_slot.agent_id);
        let mid_at_wp = walk_out_end + (at_wp_end - walk_out_end) / 2;

        // Capture destinations chosen across many cycles. Trip cycles produce
        // either AtWaypoint or AimlessAt — both contribute to destination
        // variety, so we count distinct destination x coords.
        let mut dest_xs = std::collections::HashSet::new();
        for n in 0..50u64 {
            let t = n * cycle + mid_at_wp;
            let (s, now) = slot(ActivityState::Idle, t);
            match derive(&s, now, &l) {
                Some(Pose::AtWaypoint { wp, .. }) => {
                    dest_xs.insert(l.waypoints[wp].pos.x);
                }
                Some(Pose::AimlessAt { dest }) => {
                    dest_xs.insert(dest.x);
                }
                _ => {}
            }
        }
        assert!(
            dest_xs.len() >= 2,
            "destination should vary across cycles, got {dest_xs:?}"
        );
    }
}
