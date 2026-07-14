use std::collections::HashSet;

use anyhow::{anyhow, bail, Result};

use super::mask::{build_walkable_mask, ground_rect, pantry_ground_rect};
use super::{
    anchored_top_left, approach_point, furniture_def, Anchor, Bounds, Furniture, Layout, Point,
    ReachSet, Size, WallDecor, WaypointKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutPosition {
    pub item: String,
    pub pos: Point,
}

impl LayoutPosition {
    pub fn new(item: impl Into<String>, pos: Point) -> Self {
        Self {
            item: item.into(),
            pos,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct LayoutOverrides {
    positions: Vec<LayoutPosition>,
}

impl LayoutOverrides {
    pub fn new(positions: impl IntoIterator<Item = LayoutPosition>) -> Self {
        Self {
            positions: positions.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Item {
    LoungeCouch,
    LoungeFloorLamp,
    LoungeSideTable,
    PantryCounter,
    PantryIsland,
    PantrySnackShelf,
    AisleVendingMachine,
    AislePrinter,
    MeetingSofa { room: usize, sofa: usize },
    MeetingTable { room: usize },
    Plant { index: usize },
    Wall(WallDecor),
    PodDecor { index: usize },
}

impl Item {
    fn parse(value: &str) -> Result<Self> {
        let fixed = match value {
            "lounge.couch" => Some(Self::LoungeCouch),
            "lounge.floor-lamp" => Some(Self::LoungeFloorLamp),
            "lounge.side-table" => Some(Self::LoungeSideTable),
            "pantry.counter" => Some(Self::PantryCounter),
            "pantry.island" => Some(Self::PantryIsland),
            "pantry.snack-shelf" => Some(Self::PantrySnackShelf),
            "aisle.vending-machine" => Some(Self::AisleVendingMachine),
            "aisle.printer" => Some(Self::AislePrinter),
            "plant.cubicle-west" => Some(Self::Plant { index: 0 }),
            "plant.cubicle-east" => Some(Self::Plant { index: 1 }),
            "plant.meeting-0-north" => Some(Self::Plant { index: 2 }),
            "plant.meeting-0-south" => Some(Self::Plant { index: 3 }),
            "wall.bookshelf" => Some(Self::Wall(WallDecor::Bookshelf)),
            "wall.exit-sign" => Some(Self::Wall(WallDecor::ExitSign)),
            "wall.whiteboard" => Some(Self::Wall(WallDecor::Whiteboard)),
            "wall.meeting-screen" => Some(Self::Wall(WallDecor::MeetingScreen)),
            _ => None,
        };
        if let Some(item) = fixed {
            return Ok(item);
        }
        if let Some(index) = value.strip_prefix("pod-decor.") {
            return Ok(Self::PodDecor {
                index: parse_index(value, index)?,
            });
        }
        if let Some(rest) = value.strip_prefix("meeting.") {
            let mut parts = rest.split('.');
            let room = parse_index(value, parts.next().unwrap_or_default())?;
            let role = parts.next().unwrap_or_default();
            let side = parts.next();
            if parts.next().is_some() {
                bail!("unknown layout item {value:?}");
            }
            return match (role, side) {
                ("north-sofa", None) => Ok(Self::MeetingSofa { room, sofa: 0 }),
                ("south-sofa", None) => Ok(Self::MeetingSofa { room, sofa: 1 }),
                ("table", None) => Ok(Self::MeetingTable { room }),
                _ => bail!("unknown layout item {value:?}"),
            };
        }
        bail!("unknown layout item {value:?}")
    }
}

fn parse_index(item: &str, value: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| anyhow!("invalid index in layout item {item:?}"))
}

pub(super) fn apply(layout: &mut Layout, overrides: &LayoutOverrides) -> Result<()> {
    let procedural = layout.clone();
    let mut seen = HashSet::new();
    let mut moved = Vec::with_capacity(overrides.positions.len());
    for position in &overrides.positions {
        let item = Item::parse(&position.item)?;
        if !seen.insert(item) {
            bail!("duplicate layout item {:?}", position.item);
        }
        move_item(layout, item, position.pos)
            .map_err(|e| anyhow!("layout item {:?}: {e}", position.item))?;
        moved.push((position.item.as_str(), item));
    }

    for &(name, item) in &moved {
        validate_bounds(layout, item).map_err(|e| anyhow!("layout item {name:?}: {e}"))?;
        if item_position(layout, item)? != item_position(&procedural, item)? {
            validate_collision(layout, item).map_err(|e| anyhow!("layout item {name:?}: {e}"))?;
        }
    }

    rebuild_navigation(layout);
    validate_connected(layout)?;
    validate_approaches(layout)?;
    Ok(())
}

fn item_position(layout: &Layout, item: Item) -> Result<Point> {
    match item {
        Item::LoungeCouch => layout
            .couch_sprite_center
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::LoungeFloorLamp => layout
            .floor_lamp
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::LoungeSideTable => layout
            .lounge_side_table
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::PantryCounter => waypoint_pos(layout, WaypointKind::Pantry),
        Item::PantryIsland => layout
            .pantry
            .and_then(|p| p.kitchen_island)
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::PantrySnackShelf => waypoint_pos(layout, WaypointKind::SnackShelf),
        Item::AisleVendingMachine => waypoint_pos(layout, WaypointKind::VendingMachine),
        Item::AislePrinter => waypoint_pos(layout, WaypointKind::Printer),
        Item::MeetingSofa { room, sofa } => layout
            .meeting_rooms
            .get(room)
            .and_then(|r| r.trio)
            .map(|t| t.sofas[sofa])
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::MeetingTable { room } => layout
            .meeting_rooms
            .get(room)
            .and_then(|r| r.trio)
            .map(|t| t.table)
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::Plant { index } => layout
            .plants
            .get(index)
            .map(|p| p.pos)
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::Wall(kind) => wall_pos(layout, kind),
        Item::PodDecor { index } => layout
            .pod_decor
            .get(index)
            .map(|p| p.pos)
            .ok_or_else(|| anyhow!("is not present on this floor")),
    }
}

fn move_item(layout: &mut Layout, item: Item, target: Point) -> Result<()> {
    match item {
        Item::LoungeCouch => {
            let old = layout
                .couch_sprite_center
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            shift_waypoints(layout, old, target, |w| w.kind == WaypointKind::Couch)?;
            layout.couch_sprite_center = Some(target);
        }
        Item::LoungeFloorLamp => set_option(&mut layout.floor_lamp, target)?,
        Item::LoungeSideTable => set_option(&mut layout.lounge_side_table, target)?,
        Item::PantryCounter => set_waypoint(layout, WaypointKind::Pantry, target)?,
        Item::PantryIsland => {
            let old = layout
                .pantry
                .as_ref()
                .and_then(|p| p.kitchen_island)
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            shift_waypoints(layout, old, target, |w| w.kind == WaypointKind::Island)?;
            layout
                .pantry
                .as_mut()
                .ok_or_else(|| anyhow!("is not present on this floor"))?
                .kitchen_island = Some(target);
        }
        Item::PantrySnackShelf => set_waypoint(layout, WaypointKind::SnackShelf, target)?,
        Item::AisleVendingMachine => set_waypoint(layout, WaypointKind::VendingMachine, target)?,
        Item::AislePrinter => set_waypoint(layout, WaypointKind::Printer, target)?,
        Item::MeetingSofa { room, sofa } => {
            let old = layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.sofas[sofa])
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            shift_waypoints(layout, old, target, |w| {
                w.kind == WaypointKind::MeetingSofa && w.room_id == Some(room) && w.pos.y == old.y
            })?;
            if let Some(t) = layout.meeting_rooms[room].trio.as_mut() {
                t.sofas[sofa] = target;
            }
        }
        Item::MeetingTable { room } => {
            let old = layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.table)
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            shift_waypoints(layout, old, target, |w| {
                w.kind == WaypointKind::MeetingStand && w.room_id == Some(room)
            })?;
            if let Some(t) = layout.meeting_rooms[room].trio.as_mut() {
                t.table = target;
            }
        }
        Item::Plant { index } => {
            layout
                .plants
                .get_mut(index)
                .ok_or_else(|| anyhow!("is not present on this floor"))?
                .pos = target;
        }
        Item::Wall(kind) => {
            layout
                .wall_decor
                .iter_mut()
                .find(|item| item.kind == kind)
                .ok_or_else(|| anyhow!("is not present on this floor"))?
                .pos = target;
        }
        Item::PodDecor { index } => {
            let placed = layout
                .pod_decor
                .get(index)
                .copied()
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            shift_waypoints(layout, placed.pos, target, |w| {
                matches!(
                    w.kind,
                    WaypointKind::PhoneBooth | WaypointKind::StandingDesk
                ) && w.pos == placed.pos
            })?;
            layout.pod_decor[index].pos = target;
        }
    }
    Ok(())
}

fn set_option(slot: &mut Option<Point>, target: Point) -> Result<()> {
    if slot.is_none() {
        bail!("is not present on this floor");
    }
    *slot = Some(target);
    Ok(())
}

fn set_waypoint(layout: &mut Layout, kind: WaypointKind, target: Point) -> Result<()> {
    layout
        .waypoints
        .iter_mut()
        .find(|w| w.kind == kind)
        .ok_or_else(|| anyhow!("is not present on this floor"))?
        .pos = target;
    Ok(())
}

fn shift_waypoints(
    layout: &mut Layout,
    old: Point,
    target: Point,
    mut selected: impl FnMut(&super::Waypoint) -> bool,
) -> Result<()> {
    let dx = i32::from(target.x) - i32::from(old.x);
    let dy = i32::from(target.y) - i32::from(old.y);
    for waypoint in layout.waypoints.iter_mut().filter(|w| selected(w)) {
        waypoint.pos = shifted(waypoint.pos, dx, dy)?;
    }
    Ok(())
}

fn shifted(point: Point, dx: i32, dy: i32) -> Result<Point> {
    let x = i32::from(point.x) + dx;
    let y = i32::from(point.y) + dy;
    if !(0..=i32::from(u16::MAX)).contains(&x) || !(0..=i32::from(u16::MAX)).contains(&y) {
        bail!("moves a dependent position outside the coordinate range");
    }
    Ok(Point {
        x: x as u16,
        y: y as u16,
    })
}

fn validate_bounds(layout: &Layout, item: Item) -> Result<()> {
    let buffer = Bounds {
        x: 0,
        y: 0,
        width: layout.buf_w,
        height: layout.buf_h,
    };
    let visuals = visual_rects(layout, item)?;
    let centers = centered_positions(layout, item)?;
    if !centers.is_empty() {
        debug_assert_eq!(centers.len(), visuals.len());
        for (center, &(_, size)) in centers.iter().zip(&visuals) {
            let left = i32::from(center.x) - i32::from(size.w / 2);
            let top = i32::from(center.y) - i32::from(size.h / 2);
            if left < 0
                || top < 0
                || left + i32::from(size.w) > i32::from(layout.buf_w)
                || top + i32::from(size.h) > i32::from(layout.buf_h)
            {
                bail!("visual leaves the scene bounds");
            }
        }
    }
    for rect in visuals {
        if !rect_in_bounds(rect, buffer) {
            bail!("visual leaves the scene bounds");
        }
    }
    let container = container(layout, item)?;
    for rect in ground_rects(layout, item)? {
        if !rect_in_bounds(rect, container) {
            bail!("ground leaves its allowed area");
        }
    }
    Ok(())
}

fn centered_positions(layout: &Layout, item: Item) -> Result<Vec<Point>> {
    match item {
        Item::LoungeCouch => Ok(layout
            .waypoints
            .iter()
            .filter(|w| w.kind == WaypointKind::Couch)
            .map(|w| w.pos)
            .collect()),
        Item::LoungeFloorLamp => one_point(layout.floor_lamp),
        Item::LoungeSideTable => one_point(layout.lounge_side_table),
        Item::PantryCounter => Ok(vec![waypoint_pos(layout, WaypointKind::Pantry)?]),
        Item::PantryIsland => one_point(layout.pantry.and_then(|p| p.kitchen_island)),
        Item::PantrySnackShelf => Ok(vec![waypoint_pos(layout, WaypointKind::SnackShelf)?]),
        Item::AisleVendingMachine => Ok(vec![waypoint_pos(layout, WaypointKind::VendingMachine)?]),
        Item::AislePrinter => Ok(vec![waypoint_pos(layout, WaypointKind::Printer)?]),
        Item::MeetingSofa { room, sofa } => one_point(
            layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.sofas[sofa]),
        ),
        Item::MeetingTable { room } => one_point(
            layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.table),
        ),
        Item::Plant { index } => layout
            .plants
            .get(index)
            .map(|plant| vec![plant.pos])
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::Wall(_) => Ok(Vec::new()),
        Item::PodDecor { index } => layout
            .pod_decor
            .get(index)
            .map(|decor| vec![decor.pos])
            .ok_or_else(|| anyhow!("is not present on this floor")),
    }
}

fn one_point(pos: Option<Point>) -> Result<Vec<Point>> {
    Ok(vec![
        pos.ok_or_else(|| anyhow!("is not present on this floor"))?
    ])
}

fn validate_collision(layout: &Layout, item: Item) -> Result<()> {
    if let Item::Wall(kind) = item {
        let target = visual_rects(layout, item)?[0];
        let overlaps_wall_decor = layout
            .wall_decor
            .iter()
            .filter(|decor| decor.kind != kind)
            .any(|decor| {
                super::placement::rects_overlap(
                    target,
                    table_rect(Anchor::TopLeft, decor.pos, decor.kind.furniture(), false),
                )
            });
        let overlaps_elevator = layout.door.is_some_and(|door| {
            super::placement::rects_overlap(
                target,
                (
                    door,
                    Size {
                        w: super::ELEVATOR_W,
                        h: super::ELEVATOR_H,
                    },
                ),
            )
        });
        if overlaps_wall_decor || overlaps_elevator {
            bail!("visually collides with wall decor or the elevator");
        }
    }
    let mut others = layout.clone();
    remove_item(&mut others, item);
    let mask = rebuild_mask(&others);
    for (tl, size) in ground_rects(layout, item)? {
        let collides_desk = layout.home_desks.iter().any(|&desk| {
            let def = furniture_def(Furniture::Desk);
            def.footprint.is_some_and(|fp| {
                let (desk_tl, desk_size) = ground_rect(
                    Anchor::TopLeft,
                    desk,
                    fp,
                    def.visual,
                    def.ground_x,
                    def.ground_y,
                );
                let pad = super::OBSTACLE_PAD_PX;
                let padded_tl = Point {
                    x: desk_tl.x.saturating_sub(pad),
                    y: desk_tl.y.saturating_sub(pad),
                };
                let padded = Size {
                    w: desk_size.w + pad * 2,
                    h: desk_size.h + pad * 2,
                };
                super::placement::rects_overlap((tl, size), (padded_tl, padded))
            })
        });
        if collides_desk {
            bail!("collides with a fixed desk");
        }
        for y in tl.y..tl.y + size.h {
            for x in tl.x..tl.x + size.w {
                if !mask.is_walkable(x, y) {
                    bail!("collides with a wall or another furniture item");
                }
            }
        }
    }
    Ok(())
}

fn remove_item(layout: &mut Layout, item: Item) {
    match item {
        Item::LoungeCouch => {
            layout.waypoints.retain(|w| w.kind != WaypointKind::Couch);
            layout.couch_sprite_center = None;
        }
        Item::LoungeFloorLamp => {
            layout.floor_lamp = None;
        }
        Item::LoungeSideTable => {
            layout.lounge_side_table = None;
        }
        Item::PantryCounter => layout.waypoints.retain(|w| w.kind != WaypointKind::Pantry),
        Item::PantryIsland => {
            layout.waypoints.retain(|w| w.kind != WaypointKind::Island);
            if let Some(pantry) = &mut layout.pantry {
                pantry.kitchen_island = None;
            }
        }
        Item::PantrySnackShelf => layout
            .waypoints
            .retain(|w| w.kind != WaypointKind::SnackShelf),
        Item::AisleVendingMachine => layout
            .waypoints
            .retain(|w| w.kind != WaypointKind::VendingMachine),
        Item::AislePrinter => layout.waypoints.retain(|w| w.kind != WaypointKind::Printer),
        Item::MeetingSofa { room, sofa } => {
            if let Some(trio) = layout
                .meeting_rooms
                .get_mut(room)
                .and_then(|r| r.trio.as_mut())
            {
                trio.sofas[sofa] = Point { x: 0, y: 0 };
            }
        }
        Item::MeetingTable { room } => {
            if let Some(trio) = layout
                .meeting_rooms
                .get_mut(room)
                .and_then(|r| r.trio.as_mut())
            {
                trio.table = Point { x: 0, y: 0 };
            }
        }
        Item::Plant { index } => {
            if index < layout.plants.len() {
                layout.plants.remove(index);
            }
        }
        Item::Wall(kind) => layout.wall_decor.retain(|item| item.kind != kind),
        Item::PodDecor { index } => {
            if index < layout.pod_decor.len() {
                let removed = layout.pod_decor.remove(index);
                layout.waypoints.retain(|w| w.pos != removed.pos);
            }
        }
    }
}

fn visual_rects(layout: &Layout, item: Item) -> Result<Vec<(Point, Size)>> {
    match item {
        Item::LoungeCouch => Ok(layout
            .waypoints
            .iter()
            .filter(|w| w.kind == WaypointKind::Couch)
            .map(|w| table_rect(Anchor::Center, w.pos, Furniture::Couch, false))
            .collect()),
        Item::LoungeFloorLamp => one_option(layout.floor_lamp, Furniture::FloorLamp, false),
        Item::LoungeSideTable => {
            one_option(layout.lounge_side_table, Furniture::LoungeSideTable, false)
        }
        Item::PantryCounter => {
            let pos = waypoint_pos(layout, WaypointKind::Pantry)?;
            let size = layout.pantry_counter_size();
            Ok(vec![(
                anchored_top_left(Anchor::Center, pos, size.w, size.h),
                size,
            )])
        }
        Item::PantryIsland => one_option(
            layout.pantry.and_then(|p| p.kitchen_island),
            Furniture::KitchenIsland,
            false,
        ),
        Item::PantrySnackShelf => waypoint_visual(layout, WaypointKind::SnackShelf),
        Item::AisleVendingMachine => waypoint_visual(layout, WaypointKind::VendingMachine),
        Item::AislePrinter => waypoint_visual(layout, WaypointKind::Printer),
        Item::MeetingSofa { room, sofa } => one_option(
            layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.sofas[sofa]),
            Furniture::MeetingSofaBody,
            false,
        ),
        Item::MeetingTable { room } => one_option(
            layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.table),
            Furniture::MeetingTable,
            false,
        ),
        Item::Plant { index } => {
            let plant = layout
                .plants
                .get(index)
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            Ok(vec![table_rect(
                Anchor::Center,
                plant.pos,
                plant.kind.furniture(),
                false,
            )])
        }
        Item::Wall(kind) => {
            let pos = wall_pos(layout, kind)?;
            Ok(vec![table_rect(
                Anchor::TopLeft,
                pos,
                kind.furniture(),
                false,
            )])
        }
        Item::PodDecor { index } => {
            let placed = layout
                .pod_decor
                .get(index)
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            Ok(vec![table_rect(
                Anchor::Center,
                placed.pos,
                placed.kind.furniture(),
                false,
            )])
        }
    }
}

fn ground_rects(layout: &Layout, item: Item) -> Result<Vec<(Point, Size)>> {
    match item {
        Item::LoungeCouch => Ok(layout
            .waypoints
            .iter()
            .filter(|w| w.kind == WaypointKind::Couch)
            .filter_map(|w| table_ground(Anchor::Center, w.pos, Furniture::Couch))
            .collect()),
        Item::LoungeFloorLamp => one_ground(layout.floor_lamp, Furniture::FloorLamp),
        Item::LoungeSideTable => one_ground(layout.lounge_side_table, Furniture::LoungeSideTable),
        Item::PantryCounter => Ok(vec![pantry_ground_rect(
            waypoint_pos(layout, WaypointKind::Pantry)?,
            layout.pantry_counter_size(),
        )]),
        Item::PantryIsland => one_ground(
            layout.pantry.and_then(|p| p.kitchen_island),
            Furniture::KitchenIsland,
        ),
        Item::PantrySnackShelf => waypoint_ground(layout, WaypointKind::SnackShelf),
        Item::AisleVendingMachine => waypoint_ground(layout, WaypointKind::VendingMachine),
        Item::AislePrinter => waypoint_ground(layout, WaypointKind::Printer),
        Item::MeetingSofa { room, sofa } => one_ground(
            layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.sofas[sofa]),
            Furniture::MeetingSofaBody,
        ),
        Item::MeetingTable { room } => one_ground(
            layout
                .meeting_rooms
                .get(room)
                .and_then(|r| r.trio)
                .map(|t| t.table),
            Furniture::MeetingTable,
        ),
        Item::Plant { index } => {
            let plant = layout
                .plants
                .get(index)
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            Ok(
                table_ground(Anchor::Center, plant.pos, plant.kind.furniture())
                    .into_iter()
                    .collect(),
            )
        }
        Item::Wall(kind) => {
            Ok(
                table_ground(Anchor::TopLeft, wall_pos(layout, kind)?, kind.furniture())
                    .into_iter()
                    .collect(),
            )
        }
        Item::PodDecor { index } => {
            let placed = layout
                .pod_decor
                .get(index)
                .ok_or_else(|| anyhow!("is not present on this floor"))?;
            Ok(
                table_ground(Anchor::Center, placed.pos, placed.kind.furniture())
                    .into_iter()
                    .collect(),
            )
        }
    }
}

fn one_option(pos: Option<Point>, kind: Furniture, ground: bool) -> Result<Vec<(Point, Size)>> {
    let pos = pos.ok_or_else(|| anyhow!("is not present on this floor"))?;
    if ground {
        Ok(table_ground(Anchor::Center, pos, kind)
            .into_iter()
            .collect())
    } else {
        Ok(vec![table_rect(Anchor::Center, pos, kind, false)])
    }
}

fn one_ground(pos: Option<Point>, kind: Furniture) -> Result<Vec<(Point, Size)>> {
    one_option(pos, kind, true)
}

fn waypoint_pos(layout: &Layout, kind: WaypointKind) -> Result<Point> {
    layout
        .waypoints
        .iter()
        .find(|w| w.kind == kind)
        .map(|w| w.pos)
        .ok_or_else(|| anyhow!("is not present on this floor"))
}

fn waypoint_visual(layout: &Layout, kind: WaypointKind) -> Result<Vec<(Point, Size)>> {
    Ok(vec![table_rect(
        Anchor::Center,
        waypoint_pos(layout, kind)?,
        kind.furniture(),
        false,
    )])
}

fn waypoint_ground(layout: &Layout, kind: WaypointKind) -> Result<Vec<(Point, Size)>> {
    Ok(table_ground(
        Anchor::Center,
        waypoint_pos(layout, kind)?,
        kind.furniture(),
    )
    .into_iter()
    .collect())
}

fn wall_pos(layout: &Layout, kind: WallDecor) -> Result<Point> {
    layout
        .wall_decor
        .iter()
        .find(|item| item.kind == kind)
        .map(|item| item.pos)
        .ok_or_else(|| anyhow!("is not present on this floor"))
}

fn table_rect(anchor: Anchor, pos: Point, kind: Furniture, ground: bool) -> (Point, Size) {
    let def = furniture_def(kind);
    if ground {
        return def
            .footprint
            .map(|fp| ground_rect(anchor, pos, fp, def.visual, def.ground_x, def.ground_y))
            .unwrap_or((pos, Size { w: 0, h: 0 }));
    }
    (
        anchored_top_left(anchor, pos, def.visual.w, def.visual.h),
        def.visual,
    )
}

fn table_ground(anchor: Anchor, pos: Point, kind: Furniture) -> Option<(Point, Size)> {
    let def = furniture_def(kind);
    def.footprint
        .map(|fp| ground_rect(anchor, pos, fp, def.visual, def.ground_x, def.ground_y))
}

fn container(layout: &Layout, item: Item) -> Result<Bounds> {
    match item {
        Item::LoungeCouch
        | Item::LoungeFloorLamp
        | Item::LoungeSideTable
        | Item::PodDecor { .. }
        | Item::Plant { index: 0 | 1 } => Ok(layout.cubicle_band),
        Item::PantryCounter | Item::PantryIsland | Item::PantrySnackShelf => layout
            .pantry
            .map(|p| p.bounds)
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::AisleVendingMachine | Item::AislePrinter => Ok(layout.cubicle_aisle),
        Item::MeetingSofa { room, .. } | Item::MeetingTable { room } => layout
            .meeting_room_bounds(room)
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::Plant { index: 2 | 3 } => layout
            .meeting_room_bounds(0)
            .ok_or_else(|| anyhow!("is not present on this floor")),
        Item::Plant { .. } => bail!("is not present on this floor"),
        Item::Wall(WallDecor::Whiteboard) => Ok(layout.cubicle_band),
        Item::Wall(WallDecor::Bookshelf | WallDecor::MeetingScreen) => Ok(Bounds {
            x: 0,
            y: layout.wall_band_h(),
            width: layout.buf_w,
            height: layout.top_margin - layout.wall_band_h(),
        }),
        Item::Wall(WallDecor::ExitSign | WallDecor::BulletinBoard) => Ok(Bounds {
            x: 0,
            y: 0,
            width: layout.buf_w,
            height: layout.top_margin,
        }),
    }
}

fn rect_in_bounds((tl, size): (Point, Size), bounds: Bounds) -> bool {
    tl.x >= bounds.x
        && tl.y >= bounds.y
        && tl.x + size.w <= bounds.x + bounds.width
        && tl.y + size.h <= bounds.y + bounds.height
}

fn rebuild_mask(layout: &Layout) -> pixtuoid_core::walkable::WalkableMask {
    build_walkable_mask(
        layout.buf_w,
        layout.buf_h,
        layout.top_margin,
        layout.door,
        &layout.home_desks,
        &layout.meeting_rooms,
        layout.pantry.and_then(|p| p.kitchen_island),
        &layout.waypoints,
        &layout.plants,
        layout.floor_lamp,
        layout.lounge_side_table,
        &layout.wall_decor,
        &layout.pod_decor,
        &layout.room_walls,
        layout.pantry_counter_size(),
    )
}

fn rebuild_navigation(layout: &mut Layout) {
    layout.walkable = rebuild_mask(layout);
    let seed = layout
        .door_threshold
        .or_else(|| layout.home_desks.first().copied())
        .unwrap_or(Point {
            x: layout.buf_w / 2,
            y: layout.buf_h / 2,
        });
    layout.reachable = ReachSet::from_mask(&layout.walkable, seed);
}

fn validate_connected(layout: &Layout) -> Result<()> {
    let start = layout
        .door_threshold
        .filter(|p| layout.is_walkable(p.x, p.y))
        .or_else(|| {
            (0..layout.buf_h).find_map(|y| {
                (0..layout.buf_w)
                    .find(|&x| layout.is_walkable(x, y))
                    .map(|x| Point { x, y })
            })
        })
        .ok_or_else(|| anyhow!("layout has no walkable floor"))?;
    let mut seen = vec![false; usize::from(layout.buf_w) * usize::from(layout.buf_h)];
    let index = |p: Point| usize::from(p.y) * usize::from(layout.buf_w) + usize::from(p.x);
    let mut stack = vec![start];
    seen[index(start)] = true;
    while let Some(point) = stack.pop() {
        for (dx, dy) in [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
            let x = i32::from(point.x) + dx;
            let y = i32::from(point.y) + dy;
            if x < 0 || y < 0 || x >= i32::from(layout.buf_w) || y >= i32::from(layout.buf_h) {
                continue;
            }
            let next = Point {
                x: x as u16,
                y: y as u16,
            };
            if !seen[index(next)] && layout.is_walkable(next.x, next.y) {
                seen[index(next)] = true;
                stack.push(next);
            }
        }
    }
    let disconnected = (0..layout.buf_h)
        .flat_map(|y| (0..layout.buf_w).map(move |x| Point { x, y }))
        .any(|p| layout.is_walkable(p.x, p.y) && !seen[index(p)]);
    if disconnected {
        bail!("layout disconnects the walkable floor");
    }
    Ok(())
}

fn validate_approaches(layout: &Layout) -> Result<()> {
    let origin = layout
        .home_desks
        .first()
        .copied()
        .or(layout.door_threshold)
        .ok_or_else(|| anyhow!("layout has no routing origin"))?;
    for waypoint in &layout.waypoints {
        let approach = approach_point(
            waypoint.kind.furniture(),
            waypoint.pos,
            waypoint.facing,
            layout.pantry_counter_size(),
            &layout.walkable,
            origin,
            &layout.reachable,
        );
        if approach == waypoint.pos {
            bail!("layout makes {:?} unreachable", waypoint.kind);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulary_rejects_unknown_and_duplicate_items() {
        assert!(Item::parse("wall.lava-lamp").is_err());
        let mut layout = Layout::compute_with_seed(240, 160, Some(8), 0).unwrap();
        let lamp = layout.floor_lamp.unwrap();
        let overrides = LayoutOverrides::new([
            LayoutPosition::new("lounge.floor-lamp", lamp),
            LayoutPosition::new("lounge.floor-lamp", lamp),
        ]);
        assert!(apply(&mut layout, &overrides)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
    }

    #[test]
    fn dependent_waypoints_follow_their_furniture() {
        let mut layout = Layout::compute_with_seed(240, 160, Some(8), 0).unwrap();
        let room = layout.meeting_rooms[0].trio.unwrap();
        let target = Point {
            x: room.sofas[0].x + 1,
            y: room.sofas[0].y + 1,
        };
        move_item(&mut layout, Item::MeetingSofa { room: 0, sofa: 0 }, target).unwrap();
        let seats: Vec<_> = layout
            .waypoints
            .iter()
            .filter(|w| {
                w.kind == WaypointKind::MeetingSofa && w.room_id == Some(0) && w.pos.y == target.y
            })
            .collect();
        assert_eq!(seats.len(), 3);
    }

    #[test]
    fn every_present_vocabulary_item_accepts_its_procedural_position() {
        let layout = Layout::compute_with_seed(240, 160, Some(8), 0).unwrap();
        let mut positions = vec![
            (
                "lounge.couch".to_string(),
                layout.couch_sprite_center.unwrap(),
            ),
            ("lounge.floor-lamp".to_string(), layout.floor_lamp.unwrap()),
            (
                "lounge.side-table".to_string(),
                layout.lounge_side_table.unwrap(),
            ),
        ];
        for (name, kind) in [
            ("pantry.counter", WaypointKind::Pantry),
            ("pantry.snack-shelf", WaypointKind::SnackShelf),
            ("aisle.vending-machine", WaypointKind::VendingMachine),
            ("aisle.printer", WaypointKind::Printer),
        ] {
            if let Some(waypoint) = layout.waypoints.iter().find(|w| w.kind == kind) {
                positions.push((name.to_string(), waypoint.pos));
            }
        }
        if let Some(island) = layout.pantry.and_then(|p| p.kitchen_island) {
            positions.push(("pantry.island".to_string(), island));
        }
        for (room, placed) in layout.meeting_rooms.iter().enumerate() {
            if let Some(trio) = placed.trio {
                positions.push((format!("meeting.{room}.north-sofa"), trio.sofas[0]));
                positions.push((format!("meeting.{room}.table"), trio.table));
                positions.push((format!("meeting.{room}.south-sofa"), trio.sofas[1]));
            }
        }
        for (index, plant) in layout.plants.iter().enumerate() {
            let name = match index {
                0 => "plant.cubicle-west",
                1 => "plant.cubicle-east",
                2 => "plant.meeting-0-north",
                3 => "plant.meeting-0-south",
                _ => panic!("unexpected procedural plant index {index}"),
            };
            positions.push((name.to_string(), plant.pos));
        }
        for decor in &layout.wall_decor {
            let name = match decor.kind {
                WallDecor::Bookshelf => Some("wall.bookshelf"),
                WallDecor::Whiteboard => Some("wall.whiteboard"),
                WallDecor::ExitSign => Some("wall.exit-sign"),
                WallDecor::MeetingScreen => Some("wall.meeting-screen"),
                WallDecor::BulletinBoard => None,
            };
            if let Some(name) = name {
                positions.push((name.to_string(), decor.pos));
            }
        }
        for (index, decor) in layout.pod_decor.iter().enumerate() {
            positions.push((format!("pod-decor.{index}"), decor.pos));
        }

        for (name, pos) in positions {
            let mut candidate = layout.clone();
            apply(
                &mut candidate,
                &LayoutOverrides::new([LayoutPosition::new(name.clone(), pos)]),
            )
            .unwrap_or_else(|error| panic!("{name} rejected its procedural position: {error}"));
        }
    }
}
