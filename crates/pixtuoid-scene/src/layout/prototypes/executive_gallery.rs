use super::exterior;
use crate::layout::{
    mask, Bounds, PodDecor, PodDecorItem, Point, ReachSet, SceneLayout, WallDecor, WallDecorItem,
};

pub(super) fn build(max_desks: Option<usize>) -> SceneLayout {
    let (top_margin, door, door_threshold) = exterior();
    let cap = max_desks.unwrap_or(1).min(1);
    let home_desks = [Point { x: 71, y: 56 }]
        .into_iter()
        .take(cap)
        .collect::<Vec<_>>();
    let couch = Point { x: 122, y: 83 };
    let meeting_rooms = Vec::new();
    let waypoints = Vec::new();
    let plants = Vec::new();
    let room_walls = Vec::new();
    let doorways = Vec::new();
    let pod_decor = vec![
        PodDecorItem {
            kind: PodDecor::ExecutiveMoneyPainting,
            pos: Point { x: 37, y: 16 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveMarbleFloor,
            pos: Point { x: 28, y: 68 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveMarbleFloor,
            pos: Point { x: 80, y: 68 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveMarbleFloor,
            pos: Point { x: 132, y: 68 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveBoardTable,
            pos: Point { x: 80, y: 66 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveBar,
            pos: Point { x: 143, y: 73 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveSculpture,
            pos: Point { x: 23, y: 48 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveSculpture,
            pos: Point { x: 137, y: 48 },
        },
        PodDecorItem {
            kind: PodDecor::ExecutiveChandelier,
            pos: Point { x: 80, y: 42 },
        },
    ];
    let wall_decor = vec![
        WallDecorItem {
            kind: WallDecor::Bookshelf,
            pos: Point { x: 6, y: 34 },
        },
        WallDecorItem {
            kind: WallDecor::Bookshelf,
            pos: Point { x: 15, y: 34 },
        },
        WallDecorItem {
            kind: WallDecor::Bookshelf,
            pos: Point { x: 146, y: 34 },
        },
    ];
    let cubicle_band = Bounds {
        x: 0,
        y: 28,
        width: 160,
        height: 63,
    };
    let cubicle_aisle = cubicle_band;
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
        Some(couch),
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
        doorways,
        top_margin,
        corridor: Some(cubicle_aisle),
        couch_sprite_center: Some(couch),
        walkable,
        reachable,
    }
}
