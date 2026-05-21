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

#[derive(Debug, Clone)]
pub struct Layout {
    pub buf_w: u16,
    pub buf_h: u16,
    pub cubicle_band: Rect,
    pub walkway: Rect,
    pub lounge_band: Rect,
    pub home_desks: Vec<Point>,
    pub waypoints: Vec<Point>,
}

pub const WAYPOINT_COUNT: usize = 4;
pub const DESK_W: u16 = 12;
pub const DESK_H: u16 = 6;
pub const DESK_GAP_X: u16 = 4;
pub const DESK_GAP_Y: u16 = 2;

impl Layout {
    /// Returns `None` if the buffer is too small for even one cubicle and the
    /// fixed lounge area. Caller should paint a "terminal too small" message.
    pub fn compute(_buf_w: u16, _buf_h: u16, _num_agents: usize) -> Option<Self> {
        None // implemented in Task 2
    }
}

#[cfg(test)]
mod tests {
    // tests land in Task 2.
}
