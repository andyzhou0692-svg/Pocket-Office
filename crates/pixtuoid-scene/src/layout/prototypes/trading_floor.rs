use super::exterior;
use crate::layout::{
    mask, Bounds, PodDecor, PodDecorItem, Point, ReachSet, SceneLayout, WallDecor, WallDecorItem,
};

pub(super) fn build(max_desks: Option<usize>) -> SceneLayout {
    let (top_margin, door, door_threshold) = exterior();
    let cap = max_desks.unwrap_or(8);
    let home_desks = [
        Point { x: 28, y: 39 },
        Point { x: 58, y: 39 },
        Point { x: 88, y: 39 },
        Point { x: 118, y: 39 },
        Point { x: 28, y: 62 },
        Point { x: 58, y: 62 },
        Point { x: 88, y: 62 },
        Point { x: 118, y: 62 },
    ]
    .into_iter()
    .take(cap)
    .collect::<Vec<_>>();

    let mut pod_decor = [16, 48, 80, 112]
        .into_iter()
        .map(|x| PodDecorItem {
            kind: PodDecor::TradingTicker,
            pos: Point { x, y: 30 },
        })
        .collect::<Vec<_>>();
    pod_decor.extend([
        PodDecorItem {
            kind: PodDecor::TradingBonusBoard,
            pos: Point { x: 139, y: 34 },
        },
        PodDecorItem {
            kind: PodDecor::TradingCommandWall,
            pos: Point { x: 10, y: 56 },
        },
        PodDecorItem {
            kind: PodDecor::TradingVelcroTarget,
            pos: Point { x: 151, y: 56 },
        },
        PodDecorItem {
            kind: PodDecor::TradingPhoneBank,
            pos: Point { x: 32, y: 54 },
        },
        PodDecorItem {
            kind: PodDecor::TradingPhoneBank,
            pos: Point { x: 128, y: 54 },
        },
        PodDecorItem {
            kind: PodDecor::TradingPhoneBank,
            pos: Point { x: 32, y: 78 },
        },
        PodDecorItem {
            kind: PodDecor::TradingPhoneBank,
            pos: Point { x: 128, y: 78 },
        },
        PodDecorItem {
            kind: PodDecor::TradingClutter,
            pos: Point { x: 18, y: 36 },
        },
        PodDecorItem {
            kind: PodDecor::TradingClutter,
            pos: Point { x: 50, y: 36 },
        },
        PodDecorItem {
            kind: PodDecor::TradingClutter,
            pos: Point { x: 82, y: 36 },
        },
        PodDecorItem {
            kind: PodDecor::TradingClutter,
            pos: Point { x: 114, y: 36 },
        },
    ]);
    pod_decor.extend([14, 50, 110, 146].into_iter().map(|x| PodDecorItem {
        kind: PodDecor::TradingClutter,
        pos: Point { x, y: 57 },
    }));
    pod_decor.extend([20, 52, 84, 116, 148].into_iter().map(|x| PodDecorItem {
        kind: PodDecor::TradingClutter,
        pos: Point { x, y: 89 },
    }));
    pod_decor.extend(home_desks.iter().map(|desk| PodDecorItem {
        kind: PodDecor::TradingDeskRig,
        pos: Point {
            x: desk.x + 9,
            y: desk.y + 1,
        },
    }));

    let meeting_rooms = Vec::new();
    let waypoints = Vec::new();
    let plants = Vec::new();
    let room_walls = Vec::new();
    let wall_decor = vec![WallDecorItem {
        kind: WallDecor::ExitSign,
        pos: Point { x: 151, y: 15 },
    }];
    let cubicle_band = Bounds {
        x: 0,
        y: 28,
        width: 160,
        height: 63,
    };
    let cubicle_aisle = Bounds {
        x: 0,
        y: 84,
        width: 160,
        height: 7,
    };
    let walkable = mask::build_walkable_mask(
        160,
        94,
        top_margin,
        door,
        &home_desks,
        &meeting_rooms,
        None,
        &waypoints,
        &plants,
        None,
        None,
        &wall_decor,
        &pod_decor,
        &room_walls,
        crate::layout::rooms::pantry::COMPACT_COUNTER,
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
        pantry: None,
        room_walls,
        doorways: Vec::new(),
        top_margin,
        corridor: Some(cubicle_aisle),
        couch_sprite_center: None,
        walkable,
        reachable,
    }
}
