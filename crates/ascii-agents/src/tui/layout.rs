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
/// when they arrive there. Plants are pure decor, not waypoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WaypointKind {
    Couch,
    Coffee,
    Pantry,
}

/// Wall-mounted / wall-leaning furniture, painted as decor in the top wall
/// area. Not a wander destination — agents can't walk through their own
/// cubicle row to reach the back wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WallDecor {
    Bookshelf,
    Whiteboard,
    BulletinBoard,
    ExitSign,
}

/// Variety of potted plants — each renders a different sprite. Spread
/// these around the lounge so it doesn't feel like one ficus repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlantKind {
    Ficus,
    Tall,
    Flower,
    Succulent,
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
    /// Fixed plant positions in the lounge band. Pure decor, not wander
    /// destinations. Centers (the renderer offsets by half plant size).
    pub plants: Vec<(PlantKind, Point)>,
    /// Furniture leaning against the back wall — painted into the top
    /// margin under the window band. Decor only.
    pub wall_decor: Vec<(WallDecor, Point)>,
    /// Floor lamp position in the lounge corner. Decor only.
    pub floor_lamp: Option<Point>,
    /// Door anchor (top-left corner of the door sprite). Used both as the
    /// `from` point for SessionStart walk-in animation and as decor.
    pub door: Option<Point>,
    /// Overflow workstations on the walkway floor — used when all
    /// cubicle desks are occupied. Agents with desk_index >=
    /// home_desks.len() sit here with a laptop in their lap.
    pub floor_seats: Vec<Point>,
}

pub const WAYPOINT_COUNT: usize = 3;
pub const DESK_W: u16 = 12;
pub const DESK_H: u16 = 6;
/// Hard cap on how many cubicles get painted regardless of how high
/// `max_desks` is set. Past this count, agents overflow to floor seats.
/// Keeps the cubicle band from getting wall-to-wall crowded.
pub const MAX_VISIBLE_DESKS: usize = 6;
/// Horizontal gap between cubicles. Wider than the previous 2 px so neighbor
/// desks read as distinct cubicles rather than a single long brown bar.
pub const DESK_GAP_X: u16 = 6;
/// Vertical gap between cubicle rows. Sized to clear the seated sprite's
/// 8 px head-above-desk so row N+1's desk doesn't paint over row N's character.
/// Tightened from 10 → 8 to fit more rows in the cubicle band.
pub const DESK_GAP_Y: u16 = 8;
/// Vertical reserve above the cubicle band, in buf pixels. The renderer paints
/// the top wall band (14 px tall, with windows + a clock) into this region.
/// Tightened from 28 to 20 px so the cubicles sit closer to the wall — at 28
/// there was a wide empty wood strip between the window band and the first
/// row of desks. 20 leaves just enough room (6 px) above the desk back for a
/// seated character's head to fit between desk and wall trim.
pub const TOP_MARGIN_PX: u16 = 20;

impl Layout {
    /// Returns `None` if the buffer is too small for even one cubicle and the
    /// fixed lounge area. Caller should paint a "terminal too small" message.
    pub fn compute(buf_w: u16, buf_h: u16, num_agents: usize) -> Option<Self> {
        const MIN_W: u16 = DESK_W + DESK_GAP_X * 2;
        const MIN_H: u16 = 40 + TOP_MARGIN_PX;
        if buf_w < MIN_W || buf_h < MIN_H {
            return None;
        }

        // Vertical split: TOP_MARGIN_PX reserved for the wall band, then
        // the remaining height splits between cubicle band (dynamic),
        // walkway (fixed 10%), lounge (the rest).
        //
        // The cubicle band grows from its 50% floor up to a 72% cap as
        // more agents need rendering — so 6+ sessions still get a desk
        // each instead of being clipped to the top row.
        let usable_h = buf_h - TOP_MARGIN_PX;
        let col_w_tmp = DESK_W + DESK_GAP_X;
        let row_h_tmp = DESK_H + DESK_GAP_Y;
        let cols_tmp = ((buf_w - DESK_GAP_X) / col_w_tmp).max(1);
        let needed_rows = ((num_agents as u16 + cols_tmp - 1) / cols_tmp).max(1);
        let desired_cubicle_h = needed_rows * row_h_tmp;
        let min_cubicle_h = usable_h * 50 / 100;
        let max_cubicle_h = usable_h * 72 / 100;
        let cubicle_h = desired_cubicle_h.max(min_cubicle_h).min(max_cubicle_h);
        let walkway_h = usable_h * 10 / 100;
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

        // Home desks: pack into the cubicle band as a grid. Capped at
        // MAX_VISIBLE_DESKS so the user's max_desks=16 setting doesn't
        // overcrowd the cubicle area — extra agents overflow to floor
        // seats below.
        let col_w = DESK_W + DESK_GAP_X;
        let row_h = DESK_H + DESK_GAP_Y;
        let cols = ((buf_w - DESK_GAP_X) / col_w).max(1);
        let rows = (cubicle_h / row_h).max(1);
        let max_grid = (cols * rows) as usize;
        let n = num_agents.min(max_grid).min(MAX_VISIBLE_DESKS);
        let mut home_desks = Vec::with_capacity(n);
        for i in 0..n {
            let r = (i as u16) / cols;
            let c = (i as u16) % cols;
            home_desks.push(Point {
                x: DESK_GAP_X + c * col_w,
                y: cubicle_band.y + DESK_GAP_Y + r * row_h,
            });
        }

        // Lounge waypoints: places agents actually walk to. Couch / coffee /
        // water cooler are the destinations. Bookshelf + whiteboard moved to
        // wall_decor — agents can't realistically walk through their own
        // cubicle row to reach the back wall.
        let wp_layout: &[(WaypointKind, u16, u16)] = &[
            // (kind, x_frac/100, y_frac/100 inside lounge band)
            (WaypointKind::Couch,       20, 60),  // left half
            // Pantry (formerly "water cooler") leans against the lounge's
            // back wall — top of the band — so the fridge+counter+coffee
            // strip reads as a kitchenette.
            (WaypointKind::Pantry, 55, 25),  // center-back
            (WaypointKind::Coffee,      88, 60),  // right
        ];
        let waypoints: Vec<Waypoint> = wp_layout
            .iter()
            .map(|(kind, xf, yf)| Waypoint {
                pos: Point {
                    x: buf_w * xf / 100,
                    y: lounge_band.y + lounge_band.height * yf / 100,
                },
                kind: *kind,
            })
            .collect();

        // Plants scattered through the lounge AND walkway — mix of types
        // so it doesn't read as one ficus copy-pasted. Density bumped to
        // fight the "office feels empty" complaint.
        let plants: Vec<(PlantKind, Point)> = vec![
            (PlantKind::Ficus,     Point { x: buf_w * 35 / 100, y: lounge_band.y + lounge_band.height * 55 / 100 }),
            (PlantKind::Tall,      Point { x: buf_w * 10 / 100, y: lounge_band.y + lounge_band.height * 35 / 100 }),
            (PlantKind::Flower,    Point { x: buf_w * 70 / 100, y: lounge_band.y + lounge_band.height * 60 / 100 }),
            (PlantKind::Succulent, Point { x: buf_w * 95 / 100, y: lounge_band.y + lounge_band.height * 80 / 100 }),
            (PlantKind::Tall,      Point { x: buf_w * 50 / 100, y: lounge_band.y + lounge_band.height * 80 / 100 }),
            (PlantKind::Succulent, Point { x: buf_w * 5  / 100, y: lounge_band.y + lounge_band.height * 75 / 100 }),
            (PlantKind::Flower,    Point { x: buf_w * 25 / 100, y: lounge_band.y + lounge_band.height * 85 / 100 }),
            (PlantKind::Tall,      Point { x: buf_w * 8  / 100, y: walkway.y + 2 }),
            (PlantKind::Ficus,     Point { x: buf_w * 45 / 100, y: walkway.y + 2 }),
        ];

        // Floor lamp in the right side of the lounge — adds a warm bulb
        // color that breaks up the floor.
        let floor_lamp = Some(Point {
            x: buf_w * 92 / 100,
            y: lounge_band.y + lounge_band.height * 50 / 100,
        });

        // Office door at the right end of the back wall. Tall enough that
        // the top half tucks into the wall band and the bottom half opens
        // onto the cubicle floor. SessionStart walk-in animations originate
        // from a point just below the door.
        let door = if buf_w >= 12 {
            Some(Point {
                x: buf_w.saturating_sub(10),
                y: TOP_MARGIN_PX.saturating_sub(10),
            })
        } else {
            None
        };

        // Wall decor — bookshelf + whiteboard *leaning against* the back
        // wall. Top-down view: the back of the furniture is tucked into
        // the wall sprite, so its top rows overlap the wall band (which is
        // 0..14 px). Painted AFTER the wall so it sits in front of the
        // wall trim.
        // Bookshelf stays leaning against the back wall — wall-mounted by
        // nature. Bulletin board between bookshelf and the windows. Exit
        // sign right above the door for safety-code realism. Whiteboard
        // is a portable on-wheels stand: position it in the walkway band
        // so it reads as "rolled out for standup".
        let wall_decor = vec![
            (WallDecor::Bookshelf, Point { x: buf_w * 18 / 100, y: 6 }),
            (WallDecor::BulletinBoard, Point { x: buf_w * 42 / 100, y: 8 }),
            (WallDecor::ExitSign, Point {
                x: buf_w.saturating_sub(9),
                y: TOP_MARGIN_PX.saturating_sub(13),
            }),
            (WallDecor::Whiteboard, Point {
                x: buf_w * 80 / 100,
                y: walkway.y.saturating_sub(4),
            }),
        ];

        // Floor overflow seats: scatter slots across the walkway band for
        // agents past desk capacity. Count = num_agents - home_desks.len()
        // (capped at a reasonable max so we don't pack the floor solid).
        let overflow_count = num_agents.saturating_sub(home_desks.len()).min(8);
        let floor_seats: Vec<Point> = (0..overflow_count as u16)
            .map(|i| {
                let cols = 4u16;
                let r = i / cols;
                let c = i % cols;
                Point {
                    x: 8 + c * ((buf_w.saturating_sub(16)) / cols.max(1)),
                    y: walkway.y + 1 + r * 8,
                }
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
            plants,
            wall_decor,
            floor_lamp,
            door,
            floor_seats,
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
    fn compute_places_all_waypoint_kinds() {
        let l = Layout::compute(120, 96, 1).expect("fits");
        assert_eq!(l.waypoints.len(), WAYPOINT_COUNT);
        let kinds: std::collections::HashSet<_> =
            l.waypoints.iter().map(|w| w.kind).collect();
        assert!(kinds.contains(&WaypointKind::Couch));
        assert!(kinds.contains(&WaypointKind::Pantry));
        assert!(kinds.contains(&WaypointKind::Coffee));
        for w in &l.waypoints {
            assert!(w.pos.y >= l.lounge_band.y);
            assert!(w.pos.y < l.lounge_band.y + l.lounge_band.height);
        }
        // Waypoints should be at *different* y positions — not all in one row.
        let ys: std::collections::HashSet<_> =
            l.waypoints.iter().map(|w| w.pos.y).collect();
        assert!(ys.len() >= 2, "waypoints should be at varied y, got {ys:?}");
    }

    #[test]
    fn compute_places_bookshelf_on_wall_and_whiteboard_in_walkway() {
        let l = Layout::compute(120, 96, 1).expect("fits");
        let bookshelf = l.wall_decor.iter().find(|(k, _)| *k == WallDecor::Bookshelf);
        let whiteboard = l.wall_decor.iter().find(|(k, _)| *k == WallDecor::Whiteboard);
        assert!(bookshelf.is_some(), "missing bookshelf");
        assert!(whiteboard.is_some(), "missing whiteboard");
        // Bookshelf leans against the back wall, above the cubicle band.
        assert!(bookshelf.unwrap().1.y < l.cubicle_band.y, "bookshelf below cubicles");
        // Whiteboard is freestanding/portable, lives near the walkway band.
        assert!(
            whiteboard.unwrap().1.y > l.cubicle_band.y,
            "whiteboard should be below cubicle band"
        );
    }

    #[test]
    fn compute_places_plants_in_lounge_and_walkway() {
        let l = Layout::compute(120, 96, 1).expect("fits");
        assert!(!l.plants.is_empty(), "expected at least one plant");
        // Each plant lives in the walkway or lounge band (below cubicles).
        for (_, p) in &l.plants {
            assert!(p.y >= l.walkway.y, "plant above walkway: {p:?}");
            assert!(p.y < l.lounge_band.y + l.lounge_band.height);
            assert!(p.x < l.buf_w);
        }
        let kinds: std::collections::HashSet<_> =
            l.plants.iter().map(|(k, _)| *k).collect();
        assert!(kinds.len() >= 2, "expected plant variety, got {kinds:?}");
    }

    #[test]
    fn compute_truncates_home_desks_when_more_agents_than_fit() {
        // 30 cells wide buffer, DESK_W=12 + GAP=4 = 16 per column → 1 col.
        let l = Layout::compute(30, 80, 20).expect("fits");
        assert!(l.home_desks.len() < 20, "should clamp to what fits");
        assert!(!l.home_desks.is_empty(), "should fit at least 1");
    }
}
