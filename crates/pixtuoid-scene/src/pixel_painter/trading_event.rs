use std::time::SystemTime;

use pixtuoid_core::sprite::{Rgb, RgbBuffer};

use crate::layout::{Layout, PodDecor};

const CYCLE_MS: u64 = 300_000;
const EVENT_MS: u64 = 12_000;
const GATHERING_END_MS: u64 = 2_000;
const FLIGHT_END_MS: u64 = 5_000;
const IMPACT_END_MS: u64 = 7_000;
const BOW_END_MS: u64 = 9_000;

const SUIT: Rgb = Rgb {
    r: 18,
    g: 35,
    b: 62,
};
const SHIRT: Rgb = Rgb {
    r: 236,
    g: 236,
    b: 226,
};
const SKIN: Rgb = Rgb {
    r: 222,
    g: 167,
    b: 120,
};
const HELMET: Rgb = Rgb {
    r: 218,
    g: 177,
    b: 49,
};
const SHADOW: Rgb = Rgb {
    r: 29,
    g: 32,
    b: 40,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TradingEventPhase {
    Inactive,
    Gathering,
    Flight,
    Impact,
    Bow,
    Dispersal,
}

fn cycle_ms(now: SystemTime) -> u64 {
    super::epoch_ms(now) % CYCLE_MS
}

pub(super) fn trading_event_phase(now: SystemTime) -> TradingEventPhase {
    match cycle_ms(now) {
        elapsed if elapsed >= EVENT_MS => TradingEventPhase::Inactive,
        elapsed if elapsed < GATHERING_END_MS => TradingEventPhase::Gathering,
        elapsed if elapsed < FLIGHT_END_MS => TradingEventPhase::Flight,
        elapsed if elapsed < IMPACT_END_MS => TradingEventPhase::Impact,
        elapsed if elapsed < BOW_END_MS => TradingEventPhase::Bow,
        _ => TradingEventPhase::Dispersal,
    }
}

fn rect(buf: &mut RgbBuffer, x: i32, y: i32, w: u16, h: u16, color: Rgb) {
    for dy in 0..h as i32 {
        for dx in 0..w as i32 {
            let (px, py) = (x + dx, y + dy);
            if px >= 0 && py >= 0 && px < buf.width() as i32 && py < buf.height() as i32 {
                buf.put(px as u16, py as u16, color);
            }
        }
    }
}

fn paint_trader(buf: &mut RgbBuffer, x: i32, y: i32, raised_arms: bool) {
    rect(buf, x + 2, y, 3, 3, SKIN);
    rect(buf, x + 1, y + 3, 5, 6, SUIT);
    rect(buf, x + 3, y + 3, 1, 4, SHIRT);
    if raised_arms {
        rect(buf, x, y + 1, 2, 2, SUIT);
        rect(buf, x + 6, y + 1, 2, 2, SUIT);
        rect(buf, x, y, 1, 2, SKIN);
        rect(buf, x + 7, y, 1, 2, SKIN);
    }
}

fn paint_crowd(buf: &mut RgbBuffer, dispersed: bool) {
    let positions: &[(i32, i32)] = if dispersed {
        &[(42, 54), (112, 55)]
    } else {
        &[(39, 53), (50, 55), (104, 55), (116, 53)]
    };
    for (index, &(x, y)) in positions.iter().enumerate() {
        paint_trader(buf, x, y, !dispersed && index % 2 == 0);
    }
}

fn paint_performer(buf: &mut RgbBuffer, x: i32, y: i32, horizontal: bool) {
    if horizontal {
        rect(buf, x, y + 1, 2, 3, HELMET);
        rect(buf, x + 2, y + 1, 3, 3, SKIN);
        rect(buf, x + 5, y, 8, 5, SUIT);
        rect(buf, x + 7, y + 1, 2, 3, SHIRT);
        rect(buf, x + 13, y + 1, 4, 2, SUIT);
    } else {
        rect(buf, x + 1, y, 4, 2, HELMET);
        rect(buf, x + 2, y + 2, 3, 3, SKIN);
        rect(buf, x + 1, y + 5, 5, 7, SUIT);
        rect(buf, x + 3, y + 5, 1, 5, SHIRT);
        rect(buf, x + 1, y + 12, 2, 3, SHADOW);
        rect(buf, x + 4, y + 12, 2, 3, SHADOW);
    }
}

pub(super) fn paint_trading_event(buf: &mut RgbBuffer, layout: &Layout, now: SystemTime) {
    let Some(target) = layout
        .pod_decor
        .iter()
        .find(|item| item.kind == PodDecor::TradingVelcroTarget)
        .map(|item| item.pos)
    else {
        return;
    };
    let phase = trading_event_phase(now);
    if phase == TradingEventPhase::Inactive {
        return;
    }

    paint_crowd(buf, phase == TradingEventPhase::Dispersal);
    match phase {
        TradingEventPhase::Gathering => paint_performer(buf, 69, 48, false),
        TradingEventPhase::Flight => {
            let t = (cycle_ms(now) - GATHERING_END_MS) as f32
                / (FLIGHT_END_MS - GATHERING_END_MS) as f32;
            let start_x = 57.0;
            let end_x = target.x as f32 - 18.0;
            let x = start_x + (end_x - start_x) * t;
            let y = 47.0 - 17.0 * (4.0 * t * (1.0 - t));
            paint_performer(buf, x.round() as i32, y.round() as i32, true);
        }
        TradingEventPhase::Impact => {
            paint_performer(buf, target.x as i32 - 10, target.y as i32 - 9, true)
        }
        TradingEventPhase::Bow => paint_performer(buf, target.x as i32 - 24, 49, false),
        TradingEventPhase::Dispersal => paint_performer(buf, target.x as i32 - 24, 50, false),
        TradingEventPhase::Inactive => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{SceneLayout, PREVIEW_LAYOUT_TRADING_FLOOR_SEED};
    use pixtuoid_core::sprite::{Rgb, RgbBuffer};
    use std::time::{Duration, SystemTime};

    fn at(milliseconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(milliseconds)
    }

    #[test]
    fn event_cycle_has_the_approved_six_phases() {
        assert_eq!(trading_event_phase(at(0)), TradingEventPhase::Gathering);
        assert_eq!(trading_event_phase(at(2_500)), TradingEventPhase::Flight);
        assert_eq!(trading_event_phase(at(5_500)), TradingEventPhase::Impact);
        assert_eq!(trading_event_phase(at(7_500)), TradingEventPhase::Bow);
        assert_eq!(
            trading_event_phase(at(10_000)),
            TradingEventPhase::Dispersal
        );
        assert_eq!(trading_event_phase(at(12_000)), TradingEventPhase::Inactive);
    }

    #[test]
    fn flight_paints_performer_while_inactive_frame_is_unchanged() {
        let layout =
            SceneLayout::compute_with_seed(160, 94, Some(8), PREVIEW_LAYOUT_TRADING_FLOOR_SEED)
                .unwrap();
        let bg = Rgb { r: 1, g: 2, b: 3 };
        let mut inactive = RgbBuffer::filled(160, 94, bg);
        paint_trading_event(&mut inactive, &layout, at(20_000));
        assert!(inactive.as_slice().iter().all(|pixel| *pixel == bg));

        let mut flight = RgbBuffer::filled(160, 94, bg);
        paint_trading_event(&mut flight, &layout, at(3_500));
        assert!(flight.as_slice().iter().any(|pixel| *pixel != bg));
    }
}
