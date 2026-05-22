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
    /// Top-left quadrant — small meeting room with 2 sofas + 1 table,
    /// pressed against the back wall so it shares the city-view windows.
    pub meeting_room: Option<Rect>,
    /// Bottom-left quadrant — open pantry/break-room space.
    pub pantry_room: Option<Rect>,
    /// Sofa anchor points inside the meeting room.
    pub meeting_sofas: Vec<Point>,
    /// Meeting room coffee table position.
    pub meeting_table: Option<Point>,
    /// Wall line segments (start, end) painted in fabric/drywall color
    /// to separate the quadrants. Doorway gaps already accounted for.
    pub room_walls: Vec<(Point, Point)>,
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
pub const DESK_GAP_X: u16 = 10;
/// Vertical gap between cubicle rows. Sized to clear the seated sprite's
/// 8 px head-above-desk so row N+1's desk doesn't paint over row N's character.
/// Tightened from 10 → 8 to fit more rows in the cubicle band.
pub const DESK_GAP_Y: u16 = 10;
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

        // Quadrant layout: top-left = meeting room, bottom-left = pantry,
        // top-right = cubicles, bottom-right = lounge. All four share the
        // back wall (city-view windows) at the top.
        let usable_h = buf_h - TOP_MARGIN_PX;
        let mid_x = buf_w * 42 / 100; // left side a bit narrower than right
        let mid_y_split = TOP_MARGIN_PX + usable_h / 2;

        let meeting_room = Some(Rect {
            x: 0,
            y: TOP_MARGIN_PX,
            width: mid_x,
            height: usable_h / 2,
        });
        let pantry_room = Some(Rect {
            x: 0,
            y: mid_y_split,
            width: mid_x,
            height: usable_h - usable_h / 2,
        });

        // Right side: cubicles on top, small walkway, lounge on bottom.
        let right_x = mid_x + 2; // leave 2 px for the dividing wall
        let right_w = buf_w.saturating_sub(right_x);
        let cubicle_h = usable_h * 60 / 100;
        let walkway_h = usable_h * 8 / 100;
        let lounge_h = usable_h - cubicle_h - walkway_h;
        let cubicle_band = Rect {
            x: right_x,
            y: TOP_MARGIN_PX,
            width: right_w,
            height: cubicle_h,
        };
        let walkway = Rect {
            x: right_x,
            y: TOP_MARGIN_PX + cubicle_h,
            width: right_w,
            height: walkway_h,
        };
        let lounge_band = Rect {
            x: right_x,
            y: TOP_MARGIN_PX + cubicle_h + walkway_h,
            width: right_w,
            height: lounge_h,
        };

        // Home desks pack into the right-half cubicle band only. With ~58
        // px of right-side width and 18 px col pitch we fit 3 cols.
        let col_w = DESK_W + DESK_GAP_X;
        let row_h = DESK_H + DESK_GAP_Y;
        let cols = ((right_w.saturating_sub(DESK_GAP_X)) / col_w).max(1);
        let rows = (cubicle_h / row_h).max(1);
        let max_grid = (cols * rows) as usize;
        let n = num_agents.min(max_grid).min(MAX_VISIBLE_DESKS);
        let mut home_desks = Vec::with_capacity(n);
        for i in 0..n {
            let r = (i as u16) / cols;
            let c = (i as u16) % cols;
            home_desks.push(Point {
                x: right_x + DESK_GAP_X + c * col_w,
                y: cubicle_band.y + DESK_GAP_Y + r * row_h,
            });
        }

        // Meeting room sofas (use couch sprite) + table. Two sofas facing
        // each other across a small table — the user described "two sofas
        // + one table so they can sit on sofa and work".
        let meeting_sofas = if let Some(mr) = meeting_room {
            let cx = mr.x + mr.width / 2;
            vec![
                Point { x: cx, y: mr.y + mr.height * 30 / 100 },
                Point { x: cx, y: mr.y + mr.height * 80 / 100 },
            ]
        } else {
            vec![]
        };
        let meeting_table = meeting_room.map(|mr| Point {
            x: mr.x + mr.width / 2,
            y: mr.y + mr.height / 2,
        });

        // Room walls — drywall lines separating the quadrants. Vertical
        // wall between left rooms and right side, horizontal wall on the
        // left side splitting meeting from pantry. Door gap in the middle
        // of each wall so agents can walk between rooms (we don't render
        // walking through walls — purely cosmetic for now).
        let mut room_walls = Vec::new();
        // Vertical wall (left/right divider) with a doorway in the middle.
        let v_x = mid_x;
        let v_top = TOP_MARGIN_PX;
        let v_bot = buf_h.saturating_sub(3);
        let v_door_top = TOP_MARGIN_PX + usable_h * 28 / 100;
        let v_door_bot = TOP_MARGIN_PX + usable_h * 38 / 100;
        room_walls.push((Point { x: v_x, y: v_top }, Point { x: v_x, y: v_door_top }));
        room_walls.push((Point { x: v_x, y: v_door_bot }, Point { x: v_x, y: v_bot }));
        // Horizontal wall (meeting / pantry divider, left side only).
        let h_y = mid_y_split;
        let h_door_left = mid_x * 45 / 100;
        let h_door_right = mid_x * 60 / 100;
        room_walls.push((Point { x: 0, y: h_y }, Point { x: h_door_left, y: h_y }));
        room_walls.push((Point { x: h_door_right, y: h_y }, Point { x: mid_x, y: h_y }));

        // Lounge waypoints: places agents actually walk to. Couch / coffee /
        // water cooler are the destinations. Bookshelf + whiteboard moved to
        // wall_decor — agents can't realistically walk through their own
        // cubicle row to reach the back wall.
        // Waypoints: couch + coffee in the right-side lounge band, pantry
        // moved into the bottom-left pantry_room (closer to its "kitchen"
        // identity).
        let mut waypoints: Vec<Waypoint> = vec![
            Waypoint {
                pos: Point {
                    x: lounge_band.x + lounge_band.width * 30 / 100,
                    y: lounge_band.y + lounge_band.height * 50 / 100,
                },
                kind: WaypointKind::Couch,
            },
            Waypoint {
                pos: Point {
                    x: lounge_band.x + lounge_band.width * 85 / 100,
                    y: lounge_band.y + lounge_band.height * 50 / 100,
                },
                kind: WaypointKind::Coffee,
            },
        ];
        if let Some(pr) = pantry_room {
            waypoints.push(Waypoint {
                pos: Point {
                    x: pr.x + pr.width * 60 / 100,
                    y: pr.y + pr.height * 40 / 100,
                },
                kind: WaypointKind::Pantry,
            });
        }

        // Plants scattered across all four quadrants — meeting room
        // corners, pantry room, lounge.
        let plants: Vec<(PlantKind, Point)> = vec![
            (PlantKind::Tall,      Point { x: lounge_band.x + lounge_band.width * 10 / 100, y: lounge_band.y + lounge_band.height * 30 / 100 }),
            (PlantKind::Flower,    Point { x: lounge_band.x + lounge_band.width * 55 / 100, y: lounge_band.y + lounge_band.height * 25 / 100 }),
            (PlantKind::Succulent, Point { x: lounge_band.x + lounge_band.width * 95 / 100, y: lounge_band.y + lounge_band.height * 90 / 100 }),
            (PlantKind::Ficus,     Point { x: lounge_band.x + lounge_band.width * 70 / 100, y: lounge_band.y + lounge_band.height * 90 / 100 }),
        ]
        .into_iter()
        .chain(pantry_room.into_iter().flat_map(|pr| vec![
            (PlantKind::Tall,      Point { x: pr.x + pr.width * 10 / 100, y: pr.y + pr.height * 80 / 100 }),
            (PlantKind::Succulent, Point { x: pr.x + pr.width * 90 / 100, y: pr.y + pr.height * 80 / 100 }),
        ]))
        .chain(meeting_room.into_iter().flat_map(|mr| vec![
            (PlantKind::Tall,    Point { x: mr.x + mr.width.saturating_sub(4), y: mr.y + 4 }),
        ]))
        .collect();

        // Floor lamp in the right side of the lounge — adds a warm bulb
        // color that breaks up the floor.
        // Floor lamp in the lounge corner (now right-side lounge).
        let floor_lamp = Some(Point {
            x: lounge_band.x + lounge_band.width * 95 / 100,
            y: lounge_band.y + lounge_band.height * 60 / 100,
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
            meeting_room,
            pantry_room,
            meeting_sofas,
            meeting_table,
            room_walls,
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
        // Wide buffer so the (now right-half-only) cubicle band fits all 5.
        let l = Layout::compute(160, 80, 5).expect("fits");
        assert!(
            l.home_desks.len() <= 5 && l.home_desks.len() >= 1,
            "expected up to 5 desks, got {}",
            l.home_desks.len()
        );
        for d in &l.home_desks {
            assert!(d.y >= l.cubicle_band.y);
            assert!(d.y + DESK_H <= l.cubicle_band.y + l.cubicle_band.height);
            assert!(d.x >= l.cubicle_band.x, "desk left of cubicle band: {d:?}");
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
        // Plants now scatter through all four quadrants (lounge / pantry /
        // meeting room), so just sanity-check they're within the buffer.
        for (_, p) in &l.plants {
            assert!(p.x < l.buf_w, "plant outside buffer x: {p:?}");
            assert!(p.y < l.buf_h, "plant outside buffer y: {p:?}");
        }
        let kinds: std::collections::HashSet<_> =
            l.plants.iter().map(|(k, _)| *k).collect();
        assert!(kinds.len() >= 2, "expected plant variety, got {kinds:?}");
    }

    #[test]
    fn compute_truncates_home_desks_when_more_agents_than_fit() {
        // Narrow buffer — right-half cubicle band is small, only fits a
        // couple of desks. Should clamp not crash.
        let l = Layout::compute(50, 80, 20).expect("fits");
        assert!(l.home_desks.len() < 20, "should clamp to what fits");
    }
}
