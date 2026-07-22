mod deal_team;
mod executive_gallery;
mod trading_floor;

use super::{
    mask, Bounds, Point, ReachSet, SceneLayout, ELEVATOR_H, ELEVATOR_W, MIN_TOP_MARGIN,
    WALL_BAND_TO_TOP_MARGIN,
};

const DESIGN_W: u16 = 160;
const DESIGN_H: u16 = 94;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewLayout {
    TradingFloor,
    Neighborhoods,
    ExecutiveGallery,
}

impl PreviewLayout {
    pub(super) fn from_seed(seed: u64) -> Option<Self> {
        match seed {
            super::PREVIEW_LAYOUT_TRADING_FLOOR_SEED => Some(Self::TradingFloor),
            super::PREVIEW_LAYOUT_NEIGHBORHOODS_SEED => Some(Self::Neighborhoods),
            super::PREVIEW_LAYOUT_EXECUTIVE_GALLERY_SEED => Some(Self::ExecutiveGallery),
            _ => None,
        }
    }
}

pub(super) fn compute(
    profile: PreviewLayout,
    buf_w: u16,
    buf_h: u16,
    max_desks: Option<usize>,
) -> Option<SceneLayout> {
    let layout = match profile {
        PreviewLayout::TradingFloor => trading_floor::build(max_desks),
        PreviewLayout::Neighborhoods if buf_w == DESIGN_W && buf_h == DESIGN_H => {
            deal_team::build(max_desks)
        }
        PreviewLayout::Neighborhoods => return None,
        PreviewLayout::ExecutiveGallery => executive_gallery::build(max_desks),
    };
    Some(if buf_w == DESIGN_W && buf_h == DESIGN_H {
        layout
    } else {
        fit_sparse_layout(layout, buf_w, buf_h)
    })
}

fn scale_axis(value: u16, design: u16, actual: u16) -> u16 {
    ((u32::from(value) * u32::from(actual)) / u32::from(design))
        .min(u32::from(actual.saturating_sub(1))) as u16
}

fn scale_point(point: Point, buf_w: u16, buf_h: u16) -> Point {
    Point {
        x: scale_axis(point.x, DESIGN_W, buf_w),
        y: scale_axis(point.y, DESIGN_H, buf_h),
    }
}

fn scale_bounds(bounds: Bounds, buf_w: u16, buf_h: u16) -> Bounds {
    let x = scale_axis(bounds.x, DESIGN_W, buf_w);
    let y = scale_axis(bounds.y, DESIGN_H, buf_h);
    let right = ((u32::from(bounds.x + bounds.width) * u32::from(buf_w)) / u32::from(DESIGN_W))
        .min(u32::from(buf_w)) as u16;
    let bottom = ((u32::from(bounds.y + bounds.height) * u32::from(buf_h)) / u32::from(DESIGN_H))
        .min(u32::from(buf_h)) as u16;
    Bounds {
        x,
        y,
        width: right.saturating_sub(x).max(1),
        height: bottom.saturating_sub(y).max(1),
    }
}

fn fit_sparse_layout(mut layout: SceneLayout, buf_w: u16, buf_h: u16) -> SceneLayout {
    debug_assert!(layout.meeting_rooms.is_empty());
    debug_assert!(layout.pantry.is_none());
    debug_assert!(layout.room_walls.is_empty());
    debug_assert!(layout.doorways.is_empty());

    layout.buf_w = buf_w;
    layout.buf_h = buf_h;
    layout.cubicle_band = scale_bounds(layout.cubicle_band, buf_w, buf_h);
    layout.cubicle_aisle = scale_bounds(layout.cubicle_aisle, buf_w, buf_h);
    layout.corridor = layout.corridor.map(|b| scale_bounds(b, buf_w, buf_h));
    layout.top_margin = scale_axis(layout.top_margin, DESIGN_H, buf_h).max(MIN_TOP_MARGIN);
    for point in &mut layout.home_desks {
        *point = scale_point(*point, buf_w, buf_h);
    }
    for waypoint in &mut layout.waypoints {
        waypoint.pos = scale_point(waypoint.pos, buf_w, buf_h);
    }
    for plant in &mut layout.plants {
        plant.pos = scale_point(plant.pos, buf_w, buf_h);
    }
    for decor in &mut layout.wall_decor {
        decor.pos = scale_point(decor.pos, buf_w, buf_h);
    }
    for decor in &mut layout.pod_decor {
        decor.pos = scale_point(decor.pos, buf_w, buf_h);
    }
    layout.floor_lamp = layout.floor_lamp.map(|p| scale_point(p, buf_w, buf_h));
    layout.lounge_side_table = layout
        .lounge_side_table
        .map(|p| scale_point(p, buf_w, buf_h));
    layout.door = layout.door.map(|p| scale_point(p, buf_w, buf_h));
    layout.door_threshold = layout.door_threshold.map(|p| scale_point(p, buf_w, buf_h));
    layout.couch_sprite_center = layout
        .couch_sprite_center
        .map(|p| scale_point(p, buf_w, buf_h));

    layout.walkable = mask::build_walkable_mask(
        buf_w,
        buf_h,
        layout.top_margin,
        layout.door,
        &layout.home_desks,
        &layout.meeting_rooms,
        None,
        &layout.waypoints,
        &layout.plants,
        layout.floor_lamp,
        layout.lounge_side_table,
        &layout.wall_decor,
        &layout.pod_decor,
        &layout.room_walls,
        super::rooms::pantry::COMPACT_COUNTER,
    );
    let reach_seed = layout.door_threshold.unwrap_or(Point {
        x: buf_w / 2,
        y: layout.top_margin.saturating_add(4).min(buf_h - 1),
    });
    layout.reachable = ReachSet::from_mask(&layout.walkable, reach_seed);
    layout
}

pub(super) fn exterior() -> (u16, Option<Point>, Option<Point>) {
    let top_margin = super::pct(94, 30).max(MIN_TOP_MARGIN);
    let top_wall_h = top_margin.saturating_sub(WALL_BAND_TO_TOP_MARGIN);
    let window_bottom_y = top_wall_h.saturating_sub(3);
    let door = Some(Point {
        x: DESIGN_W - ELEVATOR_W - 2,
        y: window_bottom_y + 1 - ELEVATOR_H + 2,
    });
    let threshold = door.map(|d| Point {
        x: d.x + ELEVATOR_W / 2,
        y: top_margin + 4,
    });
    (top_margin, door, threshold)
}
