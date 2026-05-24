//! Per-pose sprite anchor + breath bob + walking-position helpers.
//!
//! Pure geometry — no `RgbBuffer`, no rendering. The orchestrator calls
//! these to compute the top-left pixel where each character sprite
//! should land based on its pose (seated at desk, standing, walking,
//! at a waypoint, etc.).

use std::time::SystemTime;

use crate::tui::layout::{Point, WaypointKind, DESK_W};

pub(super) fn seated_anchor(desk: Point) -> Point {
    Point {
        x: desk.x + DESK_W.saturating_sub(8) / 2,
        y: desk.y.saturating_sub(8),
    }
}

pub(super) fn standing_at_desk_anchor(desk: Point) -> Point {
    Point {
        x: desk.x + DESK_W.saturating_sub(8) / 2,
        y: desk.y.saturating_sub(12),
    }
}

pub(super) fn walking_anchor(p: Point) -> Point {
    Point {
        x: p.x.saturating_sub(4),
        y: p.y.saturating_sub(12),
    }
}

pub(super) fn waypoint_anchor(wp: Point) -> Point {
    Point {
        x: wp.x.saturating_sub(4),
        y: wp.y.saturating_sub(12),
    }
}

/// One-pixel vertical bob on a ~2.8 s cycle with a per-agent phase offset,
/// so static (seated / standing) characters look alive instead of frozen.
/// Walking + waypoint-trip poses already animate, so we skip those.
fn breath_offset_y(agent_id: ascii_agents_core::AgentId, now: SystemTime) -> u16 {
    let elapsed_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    const CYCLE_MS: u64 = 4500;
    let offset_ms = agent_id.raw() % CYCLE_MS;
    let phase = elapsed_ms.wrapping_add(offset_ms) % CYCLE_MS;
    if phase < CYCLE_MS / 2 {
        0
    } else {
        1
    }
}

pub(super) fn with_breath(
    anchor: Point,
    agent_id: ascii_agents_core::AgentId,
    now: SystemTime,
) -> Point {
    Point {
        x: anchor.x,
        y: anchor.y.saturating_sub(breath_offset_y(agent_id, now)),
    }
}

/// Anchor for a back-view sitter on a mirror_vertical'd couch. Couch back
/// is now at the BOTTOM of the sprite, so the character's body sits
/// ENTIRELY ABOVE the couch back (head 7 px above couch center, body
/// ending right at the couch back row). Different from `couch_seat_anchor`
/// because back_couch.sprite has no transparent head/face area — its hair
/// extends across all top rows, so positioning it lower would put the
/// character's "head" overlapping the couch back row.
pub(super) fn back_couch_anchor(wp: Point) -> Point {
    Point {
        x: wp.x.saturating_sub(4),
        y: wp.y.saturating_sub(7),
    }
}

/// X-offset applied to a waypoint anchor when multiple agents land at the
/// same destination in the same cycle. rank 0 = first arrival (no offset);
/// later arrivals step aside. Couch is 14 wide so it can comfortably seat
/// two; coffee + water cooler are single-use so the queue stands well off
/// to the side.
pub(super) fn waypoint_rank_offset_x(kind: WaypointKind, rank: usize) -> i16 {
    match (kind, rank) {
        (_, 0) => 0,
        (WaypointKind::Couch, 1) => 6,
        (WaypointKind::Couch, 2) => -6,
        (WaypointKind::Couch, _) => 0,
        (_, 1) => 9,
        (_, 2) => -9,
        (_, _) => 0,
    }
}

pub(super) fn walking_position(from: Point, to: Point, t_x1000: u16) -> Point {
    let t = t_x1000 as i32;
    let dx = to.x as i32 - from.x as i32;
    let dy = to.y as i32 - from.y as i32;
    // Clamp at zero before casting to u16 — left-walking agents (to.x <
    // from.x) cross through negative x partway through their walk if the
    // animation interpolation overshoots, and a bare `as u16` cast wraps
    // silently to ~65k, blitting the sprite off-screen invisibly.
    Point {
        x: (from.x as i32 + dx * t / 1000).max(0).min(u16::MAX as i32) as u16,
        y: (from.y as i32 + dy * t / 1000).max(0).min(u16::MAX as i32) as u16,
    }
}
