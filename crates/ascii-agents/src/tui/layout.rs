//! Zone-based scene layout for the top-down office.
//!
//! Splits a buf-pixel rectangle into three vertical bands (cubicle, walkway,
//! lounge), then computes one home-desk position per agent inside the cubicle
//! band and a fixed set of named waypoints inside the lounge band. Pure
//! function — no I/O, no time, no buffer.

use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

/// Kind of a lounge waypoint — determines what pose an Idle agent strikes
/// when they arrive there. Couch = sit, Coffee = drink, OpenFloor = stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaypointKind {
    Couch,
    Coffee,
    OpenFloor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waypoint {
    pub pos: Point,
    pub kind: WaypointKind,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub buf_w: u16,
    pub buf_h: u16,
    pub cubicle_band: Rect,
    pub walkway: Rect,
    pub lounge_band: Rect,
    pub home_desks: Vec<Point>,
    pub waypoints: Vec<Waypoint>,
}

pub const WAYPOINT_COUNT: usize = 4;
pub const DESK_W: u16 = 12;
pub const DESK_H: u16 = 6;
pub const DESK_GAP_X: u16 = 4;
/// Vertical gap between cubicle rows. Sized to clear the seated sprite's
/// 8 px head-above-desk so row N+1's desk doesn't paint over row N's character.
pub const DESK_GAP_Y: u16 = 10;
/// Vertical reserve above the cubicle band, in buf pixels. The renderer paints
/// the top wall band (14 px tall, with windows + a clock) into this region.
/// Sized so a standing character (12 px tall) anchored at desk.y - 12 sits
/// comfortably below the wall trim line.
pub const TOP_MARGIN_PX: u16 = 28;

impl Layout {
    /// Returns `None` if the buffer is too small for even one cubicle and the
    /// fixed lounge area. Caller should paint a "terminal too small" message.
    pub fn compute(buf_w: u16, buf_h: u16, num_agents: usize) -> Option<Self> {
        const MIN_W: u16 = DESK_W + DESK_GAP_X * 2;
        const MIN_H: u16 = 40 + TOP_MARGIN_PX;
        if buf_w < MIN_W || buf_h < MIN_H {
            return None;
        }

        // Vertical split: TOP_MARGIN_PX reserved for the wall band, then the
        // remaining height splits 50/15/35 between cubicles / walkway / lounge.
        let usable_h = buf_h - TOP_MARGIN_PX;
        let cubicle_h = usable_h * 50 / 100;
        let walkway_h = usable_h * 15 / 100;
        let lounge_h = usable_h - cubicle_h - walkway_h;
        let cubicle_band = Rect {
            x: 0,
            y: TOP_MARGIN_PX,
            width: buf_w,
            height: cubicle_h,
        };
        let walkway = Rect {
            x: 0,
            y: TOP_MARGIN_PX + cubicle_h,
            width: buf_w,
            height: walkway_h,
        };
        let lounge_band = Rect {
            x: 0,
            y: TOP_MARGIN_PX + cubicle_h + walkway_h,
            width: buf_w,
            height: lounge_h,
        };

        // Home desks: pack into the cubicle band as a grid.
        let col_w = DESK_W + DESK_GAP_X;
        let row_h = DESK_H + DESK_GAP_Y;
        let cols = ((buf_w - DESK_GAP_X) / col_w).max(1);
        let rows = (cubicle_h / row_h).max(1);
        let max_desks = (cols * rows) as usize;
        let n = num_agents.min(max_desks);
        let mut home_desks = Vec::with_capacity(n);
        for i in 0..n {
            let r = (i as u16) / cols;
            let c = (i as u16) % cols;
            home_desks.push(Point {
                x: DESK_GAP_X + c * col_w,
                y: cubicle_band.y + DESK_GAP_Y + r * row_h,
            });
        }

        // Waypoints: 4 fixed positions across the lounge band. Index 0 sits
        // on the couch (far left, where paint_lounge_decor draws the couch),
        // index 3 stands by the coffee station (far right). The middle two
        // are open-floor spots near the plants.
        let waypoint_y = lounge_band.y + lounge_band.height / 2;
        let stride = buf_w / (WAYPOINT_COUNT as u16 + 1);
        let kinds = [
            WaypointKind::Couch,
            WaypointKind::OpenFloor,
            WaypointKind::OpenFloor,
            WaypointKind::Coffee,
        ];
        let waypoints: Vec<Waypoint> = (1..=WAYPOINT_COUNT as u16)
            .map(|i| Waypoint {
                pos: Point { x: stride * i, y: waypoint_y },
                kind: kinds[(i - 1) as usize],
            })
            .collect();

        Some(Self {
            buf_w,
            buf_h,
            cubicle_band,
            walkway,
            lounge_band,
            home_desks,
            waypoints,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_returns_none_when_buf_too_small() {
        assert!(Layout::compute(20, 20, 4).is_none());
    }

    #[test]
    fn compute_zones_are_ordered_top_to_bottom_and_nonoverlapping() {
        let l = Layout::compute(120, 80, 6).expect("fits");
        assert!(l.cubicle_band.y < l.walkway.y);
        assert!(l.walkway.y < l.lounge_band.y);
        let c_bot = l.cubicle_band.y + l.cubicle_band.height;
        let w_bot = l.walkway.y + l.walkway.height;
        assert!(c_bot <= l.walkway.y, "cubicle overlaps walkway");
        assert!(w_bot <= l.lounge_band.y, "walkway overlaps lounge");
    }

    #[test]
    fn compute_places_one_home_desk_per_agent() {
        let l = Layout::compute(120, 80, 5).expect("fits");
        assert_eq!(l.home_desks.len(), 5);
        for d in &l.home_desks {
            assert!(d.y >= l.cubicle_band.y);
            assert!(d.y + DESK_H <= l.cubicle_band.y + l.cubicle_band.height);
        }
    }

    #[test]
    fn compute_places_exactly_waypoint_count_waypoints_in_lounge() {
        let l = Layout::compute(120, 96, 1).expect("fits");
        assert_eq!(l.waypoints.len(), WAYPOINT_COUNT);
        for w in &l.waypoints {
            assert!(w.pos.y >= l.lounge_band.y);
            assert!(w.pos.y < l.lounge_band.y + l.lounge_band.height);
            assert!(w.pos.x < l.buf_w);
        }
        // Couch first, coffee last — paint_lounge_decor relies on this.
        assert_eq!(l.waypoints.first().map(|w| w.kind), Some(WaypointKind::Couch));
        assert_eq!(l.waypoints.last().map(|w| w.kind), Some(WaypointKind::Coffee));
    }

    #[test]
    fn compute_truncates_home_desks_when_more_agents_than_fit() {
        // 30 cells wide buffer, DESK_W=12 + GAP=4 = 16 per column → 1 col.
        let l = Layout::compute(30, 80, 20).expect("fits");
        assert!(l.home_desks.len() < 20, "should clamp to what fits");
        assert!(!l.home_desks.is_empty(), "should fit at least 1");
    }
}
