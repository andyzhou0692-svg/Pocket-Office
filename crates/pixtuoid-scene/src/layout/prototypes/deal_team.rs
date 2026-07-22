use super::exterior;
use crate::layout::{
    mask, Bounds, Doorway, Facing, MeetingRoom, MeetingTrio, PantryRoom, PlantItem, PlantKind,
    Point, ReachSet, SceneLayout, Size, WallDecor, WallDecorItem, WallSegment, Waypoint,
    WaypointKind,
};

pub(super) fn build(max_desks: Option<usize>) -> SceneLayout {
    let (top_margin, door, door_threshold) = exterior();
    let cap = max_desks.unwrap_or(8);
    let home_desks = [
        Point { x: 8, y: 37 },
        Point { x: 32, y: 37 },
        Point { x: 110, y: 37 },
        Point { x: 134, y: 37 },
        Point { x: 8, y: 77 },
        Point { x: 32, y: 77 },
        Point { x: 110, y: 77 },
        Point { x: 134, y: 77 },
    ]
    .into_iter()
    .take(cap)
    .collect::<Vec<_>>();
    let island = Point { x: 80, y: 61 };
    let counter_size = Size { w: 20, h: 8 };
    let pantry = Some(PantryRoom {
        bounds: Bounds {
            x: 0,
            y: 28,
            width: 160,
            height: 63,
        },
        counter_size,
        kitchen_island: Some(island),
    });
    let mut waypoints = vec![
        Waypoint {
            pos: Point { x: 67, y: 61 },
            kind: WaypointKind::Island,
            facing: Facing::East,
            room_id: None,
        },
        Waypoint {
            pos: Point { x: 93, y: 61 },
            kind: WaypointKind::Island,
            facing: Facing::West,
            room_id: None,
        },
        Waypoint {
            pos: Point { x: 75, y: 61 },
            kind: WaypointKind::Island,
            facing: Facing::South,
            room_id: None,
        },
        Waypoint {
            pos: Point { x: 85, y: 61 },
            kind: WaypointKind::Island,
            facing: Facing::South,
            room_id: None,
        },
        Waypoint {
            pos: Point { x: 14, y: 60 },
            kind: WaypointKind::Pantry,
            facing: Facing::South,
            room_id: None,
        },
        Waypoint {
            pos: Point { x: 148, y: 60 },
            kind: WaypointKind::Printer,
            facing: Facing::South,
            room_id: None,
        },
    ];
    let trio = MeetingTrio {
        sofas: [Point { x: 80, y: 34 }, Point { x: 80, y: 47 }],
        table: Point { x: 80, y: 40 },
    };
    let meeting_rooms = vec![MeetingRoom {
        bounds: Bounds {
            x: 61,
            y: 29,
            width: 38,
            height: 23,
        },
        trio: Some(trio),
    }];
    for (dx, facing) in [(-6, Facing::South), (0, Facing::South), (6, Facing::South)] {
        waypoints.push(Waypoint {
            pos: Point {
                x: trio.sofas[0].x.saturating_add_signed(dx),
                y: trio.sofas[0].y,
            },
            kind: WaypointKind::MeetingSofa,
            facing,
            room_id: Some(0),
        });
    }
    for (dx, facing) in [(-6, Facing::North), (0, Facing::North), (6, Facing::North)] {
        waypoints.push(Waypoint {
            pos: Point {
                x: trio.sofas[1].x.saturating_add_signed(dx),
                y: trio.sofas[1].y,
            },
            kind: WaypointKind::MeetingSofa,
            facing,
            room_id: Some(0),
        });
    }
    for (dx, facing) in [(-9, Facing::East), (9, Facing::West)] {
        waypoints.push(Waypoint {
            pos: Point {
                x: trio.table.x.saturating_add_signed(dx),
                y: trio.table.y,
            },
            kind: WaypointKind::MeetingStand,
            facing,
            room_id: Some(0),
        });
    }
    let plants = vec![
        PlantItem {
            kind: PlantKind::Flower,
            pos: Point { x: 5, y: 43 },
        },
        PlantItem {
            kind: PlantKind::Succulent,
            pos: Point { x: 155, y: 43 },
        },
        PlantItem {
            kind: PlantKind::Tall,
            pos: Point { x: 5, y: 84 },
        },
        PlantItem {
            kind: PlantKind::Flower,
            pos: Point { x: 155, y: 84 },
        },
    ];
    let room_walls = vec![
        wall(56, 28, 56, 40),
        wall(0, 49, 42, 49),
        wall(104, 28, 104, 40),
        wall(118, 49, 159, 49),
        wall(56, 79, 56, 90),
        wall(0, 68, 42, 68),
        wall(104, 79, 104, 90),
        wall(118, 68, 159, 68),
        wall(61, 29, 99, 29),
        wall(61, 29, 61, 52),
        wall(99, 29, 99, 52),
        wall(61, 52, 74, 52),
        wall(86, 52, 99, 52),
    ];
    let doorways = vec![Doorway {
        start: Point { x: 74, y: 52 },
        end: Point { x: 86, y: 52 },
    }];
    let wall_decor = vec![
        WallDecorItem {
            kind: WallDecor::ExitSign,
            pos: Point { x: 151, y: 15 },
        },
        WallDecorItem {
            kind: WallDecor::MeetingScreen,
            pos: Point { x: 62, y: 30 },
        },
    ];
    let pod_decor = Vec::new();
    let cubicle_band = Bounds {
        x: 0,
        y: 28,
        width: 160,
        height: 63,
    };
    let cubicle_aisle = Bounds {
        x: 0,
        y: 55,
        width: 160,
        height: 12,
    };
    let walkable = mask::build_walkable_mask(
        160,
        94,
        top_margin,
        door,
        &home_desks,
        &meeting_rooms,
        Some(island),
        &waypoints,
        &plants,
        None,
        None,
        &wall_decor,
        &pod_decor,
        &room_walls,
        counter_size,
    );
    let reachable =
        ReachSet::from_mask(&walkable, door_threshold.unwrap_or(Point { x: 80, y: 40 }));

    SceneLayout {
        buf_w: 160,
        buf_h: 94,
        cubicle_band,
        cubicle_aisle,
        home_desks,
        waypoints,
        plants,
        wall_decor,
        pod_decor,
        floor_lamp: None,
        lounge_side_table: None,
        door,
        door_threshold,
        meeting_rooms,
        pantry,
        room_walls,
        doorways,
        top_margin,
        corridor: Some(cubicle_aisle),
        couch_sprite_center: None,
        walkable,
        reachable,
    }
}

const fn wall(x0: u16, y0: u16, x1: u16, y1: u16) -> WallSegment {
    WallSegment {
        start: Point { x: x0, y: y0 },
        end: Point { x: x1, y: y1 },
    }
}
