//! Background pass — depth-independent floor, walls, windows, skyline,
//! clock, corridor runner, entry mat, time-of-day overlays, ceiling
//! light pools, lamp halo, floor shadows, and weather effects.
//!
//! Everything here paints BEFORE the y-sorted entity pass. Helpers are
//! `pub(super)` so the orchestrator (`pixel_painter/mod.rs`) can call
//! them in the order it wants.

use std::time::SystemTime;

use ascii_agents_core::sprite::{Rgb, RgbBuffer};

use super::palette::{blend, lerp_rgb};

use crate::tui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Weather {
    Clear,
    Rain,
    Storm,
    Snow,
    Fog,
    Overcast,
    Windy,
}

pub(super) fn weather_state(now: SystemTime) -> Weather {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cycle = secs / 600;
    let mut h = cycle.wrapping_add(0x9e37_79b9_7f4a_7c15);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    match h % 14 {
        0..=5 => Weather::Clear,
        6..=7 => Weather::Rain,
        8 => Weather::Storm,
        9 => Weather::Snow,
        10 => Weather::Fog,
        11..=12 => Weather::Overcast,
        _ => Weather::Windy,
    }
}

pub(super) fn sunset_strength(now: SystemTime) -> f32 {
    use chrono::Timelike;
    let unix_now = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let local = chrono::DateTime::<chrono::Local>::from(std::time::UNIX_EPOCH + unix_now);
    let h = local.hour() as f32 + local.minute() as f32 / 60.0;
    super::palette::bell(h, 18.0, 1.5).max(super::palette::bell(h, 6.5, 1.0))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_floor_and_walls(
    buf: &mut RgbBuffer,
    buf_w: u16,
    buf_h: u16,
    now: SystemTime,
    look: &TimeOfDayLook,
    top_wall_h: u16,
    skip_window_x_range: Option<(u16, u16)>,
    theme: &Theme,
    altitude: f32,
) {
    let window_frame = theme.surface.window_frame;
    let carpet_base = theme.surface.carpet_base;
    let carpet_light = theme.surface.carpet_light;
    let carpet_dark = theme.surface.carpet_dark;
    let wall = theme.surface.wall;
    let wall_trim_color = theme.surface.wall_trim;

    for y in 0..buf_h {
        for x in 0..buf_w {
            let hash = (x as u32)
                .wrapping_mul(73)
                .wrapping_add((y as u32).wrapping_mul(151))
                ^ ((x as u32).wrapping_mul(11) ^ (y as u32).wrapping_mul(37));
            let color = match hash % 17 {
                0 | 1 => carpet_light,
                2 | 3 => carpet_dark,
                _ => carpet_base,
            };
            buf.put(x, y, color);
        }
    }
    for y in 0..top_wall_h.min(buf_h) {
        for x in 0..buf_w {
            buf.put(x, y, wall);
        }
    }

    // Floor-to-ceiling windows: 落地窗 — height grows with the wall band so
    // taller terminals get dramatic floor-to-ceiling glass. Width stays
    // fixed (mullion every 22 px) so the skyline detail reads consistently.
    const WINDOW_W: u16 = 22;
    const WINDOW_GAP: u16 = 3;
    let window_y: u16 = 1;
    let window_h: u16 = top_wall_h.saturating_sub(2).max(8);
    let weather = weather_state(now);
    let mut x = 3u16;
    let mut idx: u32 = 0;
    while x + WINDOW_W + 2 <= buf_w {
        // Skip any window whose x-range overlaps the elevator door —
        // the elevator sits in the wall and would otherwise show the
        // window's glass + skyline behind its frame.
        let overlaps_door =
            skip_window_x_range.is_some_and(|(dx0, dx1)| x < dx1 && x + WINDOW_W > dx0);
        if !overlaps_door {
            paint_floor_to_ceiling_window(
                buf,
                x,
                window_y,
                WINDOW_W,
                window_h,
                window_frame,
                look,
                idx as u16,
                now,
                theme,
                weather,
                altitude,
            );
            if look.spill_strength > 0.0 {
                paint_window_light_spill(
                    buf,
                    x,
                    WINDOW_W,
                    top_wall_h,
                    look.spill_strength,
                    look.spill_slant,
                    theme,
                );
            }
        }
        x += WINDOW_W + WINDOW_GAP;
        idx += 1;
    }

    // Wall trim line at the bottom of the wall band.
    let trim_y = top_wall_h.saturating_sub(1);
    if trim_y < buf_h {
        for x in 0..buf_w {
            buf.put(x, trim_y, wall_trim_color);
        }
    }
}

/// Static "is this building window lit?" decision — independent of time.
/// Deterministic hash of (window_idx, dx, dy) so each building's window
/// pattern is stable across frames; only `city_dot_twinkle` animates
/// on top. ~75% of grid slots are lit so the city reads as "alive at
/// night" without every single window being on.
fn city_dot_lit(window_idx: u16, dx: u16, dy: u16) -> bool {
    let mut h = (window_idx as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h ^= (dx as u64).wrapping_mul(0xc6a4_a793_5bd1_e995);
    h ^= (dy as u64).wrapping_mul(0x1656_67b1_9e37_79b9);
    h ^= h >> 17;
    (h % 100) < 75
}

/// Per-dot twinkle: each city-window dot has its own ~600-1400ms cycle and
/// each cycle rerolls on/off via a deterministic hash. Bias toward "on" so
/// the skyline is mostly lit with the occasional dot blinking off.
fn city_dot_twinkle(window_idx: u16, dx: u16, dy: u16, now: SystemTime) -> bool {
    let now_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let dot_seed = (window_idx as u64).wrapping_mul(31)
        ^ (dx as u64).wrapping_mul(131)
        ^ (dy as u64).wrapping_mul(521);
    let cycle_ms = 6000 + (dot_seed % 8000);
    let phase = now_ms / cycle_ms;
    let hash = dot_seed
        .wrapping_add(phase)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (hash % 10) < 7
}

/// Warm sunlight tint spilling onto the floor below a window. Trapezoid
/// shape (widens by 1 px every 2 rows) blended with the existing floor so
/// it reads as "light through window" not "yellow rectangle". `intensity`
/// (0..1) scales with daylight — zero at night so no spill paints.
/// `slant_per_row` shifts the spill horizontally per row going down —
/// positive = rightward (morning sun in the east casts light right), negative
/// = leftward (evening sun in the west casts light left).
fn paint_window_light_spill(
    buf: &mut RgbBuffer,
    window_x: u16,
    window_w: u16,
    top_y: u16,
    intensity: f32,
    slant_per_row: f32,
    theme: &Theme,
) {
    let warm = theme.lighting.sun_spill;
    const DEPTH: u16 = 12;
    let fade_start = 0.32 * intensity;
    for dy in 0..DEPTH {
        let widen = (dy / 2).min(3);
        let shift = (slant_per_row * dy as f32).round() as i32;
        let base_x = (window_x as i32 + shift).max(0) as u16;
        let start_x = base_x.saturating_sub(widen);
        let end_x = (base_x + window_w + widen).min(buf.width);
        let y = top_y + dy;
        if y >= buf.height {
            break;
        }
        let strength = fade_start * (1.0 - dy as f32 / DEPTH as f32);
        for x in start_x..end_x {
            let cur = buf.get(x, y);
            buf.put(
                x,
                y,
                Rgb(
                    blend(cur.0, warm.0, strength),
                    blend(cur.1, warm.1, strength),
                    blend(cur.2, warm.2, strength),
                ),
            );
        }
    }
}

/// Window glass color + spill intensity + spill slant for the current local
/// hour. `spill_slant` is x-shift per row going down: positive = rightward
/// (morning sun in the east), negative = leftward (evening sun in the west).
/// `darkness` is 1 - daylight, used to drive artificial-light effects.
pub(super) struct TimeOfDayLook {
    glass_a: Rgb,
    glass_b: Rgb,
    pub(super) spill_strength: f32,
    spill_slant: f32,
    pub(super) darkness: f32,
}

pub(super) fn time_of_day_look(now: SystemTime, theme: &Theme) -> TimeOfDayLook {
    use chrono::Timelike;
    let unix_now = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let local = chrono::DateTime::<chrono::Local>::from(std::time::UNIX_EPOCH + unix_now);
    let h = local.hour() as f32 + local.minute() as f32 / 60.0;

    // Daylight intensity: full from 8 to 17, smooth ramp 5..8 and 17..20.
    let day = if !(5.0..20.0).contains(&h) {
        0.0
    } else if h < 8.0 {
        (h - 5.0) / 3.0
    } else if h < 17.0 {
        1.0
    } else {
        1.0 - (h - 17.0) / 3.0
    };

    // Twilight bell at dawn (~6.5) and dusk (~18.5) — adds orange/pink
    // tint that the cyan↔dark-blue base doesn't capture.
    let twilight = super::palette::bell(h, 6.5, 1.5).max(super::palette::bell(h, 18.5, 1.5));

    let day_a = theme.lighting.day_sky_a;
    let day_b = theme.lighting.day_sky_b;
    let night_a = theme.lighting.night_sky_a;
    let night_b = theme.lighting.night_sky_b;
    let twilight_a = theme.lighting.twilight_a;
    let twilight_b = theme.lighting.twilight_b;

    let glass_a = lerp_rgb(lerp_rgb(night_a, day_a, day), twilight_a, twilight * 0.5);
    let glass_b = lerp_rgb(lerp_rgb(night_b, day_b, day), twilight_b, twilight * 0.5);

    // Spill slant: ±0.7 px per row at peak hours (6am leftmost, 6pm
    // rightmost), zero at noon. Conventional read: morning sun on the east
    // (right of image) casts light westward (leftward shift); evening sun
    // on the west casts eastward (rightward shift).
    let slant = if h < 12.0 {
        -((12.0 - h) / 6.0).clamp(0.0, 1.0) * 0.7
    } else {
        ((h - 12.0) / 6.0).clamp(0.0, 1.0) * 0.7
    };

    TimeOfDayLook {
        glass_a,
        glass_b,
        spill_strength: day,
        spill_slant: slant,
        darkness: 1.0 - day,
    }
}

/// Multiplicative dim applied to floor pixels at night. Pulls everything
/// toward a dark navy so the artificial-light pools have something to
/// stand out against. `strength` is 0..1 (no dim..full dim).
pub(super) fn dim_floor_overlay(
    buf: &mut RgbBuffer,
    top_y: u16,
    bottom_y: u16,
    strength: f32,
    theme: &Theme,
) {
    let night_tint = theme.lighting.night_tint;
    let s = strength.clamp(0.0, 0.55);
    for y in top_y..bottom_y.min(buf.height) {
        for x in 0..buf.width {
            let cur = buf.get(x, y);
            buf.put(
                x,
                y,
                Rgb(
                    blend(cur.0, night_tint.0, s),
                    blend(cur.1, night_tint.1, s),
                    blend(cur.2, night_tint.2, s),
                ),
            );
        }
    }
}

/// Elliptical "ceiling fluorescent" pool of pale warm light on the floor.
/// Blended additively (toward pool color) with a quadratic falloff from
/// center to edge so it reads as a soft round patch, not a stamped oval.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_ceiling_pool(
    buf: &mut RgbBuffer,
    cx: u16,
    cy: u16,
    half_w: u16,
    half_h: u16,
    strength: f32,
    theme: &Theme,
) {
    let pool = theme.lighting.ceiling_pool;
    if half_w == 0 || half_h == 0 || strength <= 0.0 {
        return;
    }
    let min_x = cx.saturating_sub(half_w);
    let max_x = (cx + half_w).min(buf.width);
    let min_y = cy.saturating_sub(half_h);
    let max_y = (cy + half_h).min(buf.height);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let nx = (x as f32 - cx as f32) / half_w as f32;
            let ny = (y as f32 - cy as f32) / half_h as f32;
            let r2 = nx * nx + ny * ny;
            if r2 > 1.0 {
                continue;
            }
            let t = (1.0 - r2) * strength;
            let cur = buf.get(x, y);
            buf.put(
                x,
                y,
                Rgb(
                    blend(cur.0, pool.0, t),
                    blend(cur.1, pool.1, t),
                    blend(cur.2, pool.2, t),
                ),
            );
        }
    }
}

/// Warm radial halo around the floor lamp — only visible at night.
pub(super) fn paint_floor_lamp_halo(
    buf: &mut RgbBuffer,
    cx: u16,
    cy: u16,
    strength: f32,
    theme: &Theme,
) {
    let warm = theme.lighting.floor_lamp_halo;
    const RADIUS: u16 = 11;
    if strength <= 0.0 {
        return;
    }
    let min_x = cx.saturating_sub(RADIUS);
    let max_x = (cx + RADIUS).min(buf.width);
    let min_y = cy.saturating_sub(RADIUS);
    let max_y = (cy + RADIUS).min(buf.height);
    let r2max = (RADIUS as f32) * (RADIUS as f32);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let dx = x as f32 - cx as f32;
            let dy = y as f32 - cy as f32;
            let r2 = dx * dx + dy * dy;
            if r2 > r2max {
                continue;
            }
            let t = (1.0 - (r2 / r2max).sqrt()) * strength;
            let cur = buf.get(x, y);
            buf.put(
                x,
                y,
                Rgb(
                    blend(cur.0, warm.0, t),
                    blend(cur.1, warm.1, t),
                    blend(cur.2, warm.2, t),
                ),
            );
        }
    }
}

/// Floor-to-ceiling window with frame, mullion, and a procedural city view
/// inside the glass. Sky gradient at top blends with time-of-day glass
/// colors; the lower portion shows building silhouettes whose "windows"
/// (1-pixel dots) light up at night and twinkle on a per-dot cycle so the
/// skyline reads as alive instead of stamped.
#[allow(clippy::too_many_arguments)]
fn paint_floor_to_ceiling_window(
    buf: &mut RgbBuffer,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    frame: Rgb,
    look: &TimeOfDayLook,
    window_idx: u16,
    now: SystemTime,
    theme: &Theme,
    weather: Weather,
    altitude: f32,
) {
    let building_dark = theme.office.building_dark;
    let building_light = theme.office.building_light;
    let cw = theme.office.city_lit_windows;
    let dark_window = theme.office.city_dark_window;

    let lit_strength = look.darkness.max(0.5).clamp(0.0, 1.0);
    let lit_colors: [Rgb; 3] = [
        lerp_rgb(dark_window, cw[0], lit_strength),
        lerp_rgb(dark_window, cw[1], lit_strength),
        lerp_rgb(dark_window, cw[2], lit_strength),
    ];
    let building = lerp_rgb(building_light, building_dark, look.darkness);

    // Skyline silhouette as a 0..15 PATTERN; the actual pixel height is
    // computed per-window so the skyline auto-scales with the glass
    // height. On a 12-px-tall window the buildings are 3..7 px, on a
    // 50-px-tall window they fill 12..24 px — same visual proportion.
    const SKYLINE_PATTERN: &[u8] = &[8, 14, 11, 15, 6, 13, 9, 12, 7, 15, 10, 13];
    const PATTERN_MAX: u16 = 15;
    let glass_h = h.saturating_sub(2);
    let alt_shrink = (glass_h as f32 * 0.3 * altitude) as u16;
    let min_bh = (glass_h / 5).saturating_sub(alt_shrink).max(2);
    let max_bh = (glass_h * 50 / 100)
        .saturating_sub(alt_shrink)
        .max(min_bh + 3);
    let bh_range = max_bh.saturating_sub(min_bh);
    let sky_norm = (glass_h as f32) * 0.7;
    let sky_row: Vec<Rgb> = (0..glass_h)
        .map(|gy| {
            let sky_t = (gy as f32 / sky_norm).min(1.0);
            lerp_rgb(look.glass_b, look.glass_a, sky_t)
        })
        .collect();

    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= buf.width || py >= buf.height {
                continue;
            }
            let on_edge = dx == 0 || dx == w - 1 || dy == 0 || dy == h - 1;
            let on_mullion = dx == w / 2 || dy == h * 7 / 10;
            if on_edge || on_mullion {
                buf.put(px, py, frame);
                continue;
            }
            let glass_dx = dx - 1;
            let glass_dy = dy - 1;
            let pat_idx = ((glass_dx + window_idx * 3) % SKYLINE_PATTERN.len() as u16) as usize;
            let pat = SKYLINE_PATTERN[pat_idx] as u16;
            let building_h = min_bh + (pat * bh_range) / PATTERN_MAX;
            let in_building = glass_dy >= glass_h.saturating_sub(building_h);

            if in_building {
                let bldg_y = glass_dy - (glass_h - building_h);
                // Lit-window dots arranged on a 2-px grid (every other
                // column + every other row of the building). Per-dot
                // lit/unlit decision is hashed from (col, row, win_idx)
                // so the same building always shows the same pattern;
                // ~70 % of grid slots are lit at night. Twinkle animates
                // the lit ones on independent cycles.
                let on_grid = glass_dx % 2 == 1 && bldg_y % 2 == 1;
                let lit_base = on_grid && city_dot_lit(window_idx, glass_dx, bldg_y);
                if lit_base && city_dot_twinkle(window_idx, glass_dx, bldg_y, now) {
                    let dot_color = match (glass_dx.wrapping_add(bldg_y)) % 5 {
                        0 => lit_colors[1],
                        1 => lit_colors[2],
                        _ => lit_colors[0],
                    };
                    buf.put(px, py, dot_color);
                } else {
                    buf.put(px, py, building);
                }
            } else {
                buf.put(px, py, sky_row[glass_dy as usize]);
            }
        }
    }

    let elapsed_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    match weather {
        Weather::Rain => {
            let glass_x0 = x + 1;
            let glass_y0 = y + 1;
            let gw = w.saturating_sub(2);
            let gh = h.saturating_sub(2);
            for streak in 0..4u64 {
                let seed = window_idx as u64 * 7 + streak;
                let sx = (seed.wrapping_mul(0x9e37_79b9) % gw as u64) as u16;
                let speed = 60 + (seed.wrapping_mul(0x4f6c_dd1d) % 50);
                let offset = seed.wrapping_mul(0x85eb_ca6b) % (gh as u64).max(1);
                let phase = (elapsed_ms / speed + offset) % gh as u64;
                let len = 3 + (seed % 2) as u16;
                let px = glass_x0 + sx;
                for dy in 0..len {
                    let py = glass_y0 + ((phase as u16 + dy) % gh);
                    if px < buf.width && py < buf.height {
                        let alpha = 0.35 - (dy as f32 / len as f32) * 0.15;
                        let cur = buf.get(px, py);
                        buf.put(
                            px,
                            py,
                            Rgb(
                                blend(cur.0, 210, alpha),
                                blend(cur.1, 220, alpha),
                                blend(cur.2, 240, alpha),
                            ),
                        );
                    }
                }
            }
        }
        Weather::Storm => {
            let glass_x0 = x + 1;
            let glass_y0 = y + 1;
            let gw = w.saturating_sub(2);
            let gh = h.saturating_sub(2);
            for streak in 0..6u64 {
                let seed = window_idx as u64 * 7 + streak;
                let sx = (seed.wrapping_mul(0x9e37_79b9) % gw as u64) as u16;
                let speed = 40 + (seed.wrapping_mul(0x4f6c_dd1d) % 40);
                let offset = seed.wrapping_mul(0x85eb_ca6b) % (gh as u64).max(1);
                let phase = (elapsed_ms / speed + offset) % gh as u64;
                let len = 4 + (seed % 3) as u16;
                let px = glass_x0 + sx;
                for dy in 0..len {
                    let py = glass_y0 + ((phase as u16 + dy) % gh);
                    if px < buf.width && py < buf.height {
                        let alpha = 0.6 - (dy as f32 / len as f32) * 0.3;
                        buf.put(
                            px,
                            py,
                            Rgb(
                                blend(buf.get(px, py).0, 210, alpha),
                                blend(buf.get(px, py).1, 220, alpha),
                                blend(buf.get(px, py).2, 245, alpha),
                            ),
                        );
                    }
                }
            }
            let flash_phase = elapsed_ms % 6000;
            if flash_phase < 50 {
                for dy in 1..h.saturating_sub(1) {
                    for dx in 1..w.saturating_sub(1) {
                        let px = x + dx;
                        let py = y + dy;
                        if px < buf.width && py < buf.height {
                            let cur = buf.get(px, py);
                            buf.put(
                                px,
                                py,
                                Rgb(
                                    blend(cur.0, 255, 0.35),
                                    blend(cur.1, 255, 0.35),
                                    blend(cur.2, 255, 0.35),
                                ),
                            );
                        }
                    }
                }
            }
        }
        Weather::Snow => {
            let glass_x0 = x + 1;
            let glass_y0 = y + 1;
            let gw = w.saturating_sub(2);
            let gh = h.saturating_sub(2);
            for flake in 0..3u64 {
                let seed = window_idx as u64 * 11 + flake;
                let sx = (seed.wrapping_mul(0x517c_c1b7) % gw as u64) as u16;
                let speed = 150 + (seed.wrapping_mul(0x4f6c_dd1d) % 100);
                let offset = seed.wrapping_mul(0x85eb_ca6b) % (gh as u64).max(1);
                let phase = (elapsed_ms / speed + offset) % gh as u64;
                let wiggle = if (elapsed_ms / 400 + seed.wrapping_mul(0x9e37)) % 2 == 0 {
                    0
                } else {
                    1
                };
                let px = glass_x0 + (sx + wiggle) % gw;
                let py = glass_y0 + phase as u16;
                if px < buf.width && py < buf.height {
                    buf.put(px, py, Rgb(240, 240, 250));
                }
            }
        }
        Weather::Fog => {
            for dy in 1..h.saturating_sub(1) {
                for dx in 1..w.saturating_sub(1) {
                    let px = x + dx;
                    let py = y + dy;
                    if px < buf.width && py < buf.height {
                        let cur = buf.get(px, py);
                        buf.put(
                            px,
                            py,
                            Rgb(
                                blend(cur.0, 160, 0.25),
                                blend(cur.1, 165, 0.25),
                                blend(cur.2, 175, 0.25),
                            ),
                        );
                    }
                }
            }
        }
        Weather::Overcast => {
            for dy in 1..h.saturating_sub(1) {
                for dx in 1..w.saturating_sub(1) {
                    let px = x + dx;
                    let py = y + dy;
                    if px < buf.width && py < buf.height {
                        let cur = buf.get(px, py);
                        buf.put(
                            px,
                            py,
                            Rgb(
                                blend(cur.0, 100, 0.2),
                                blend(cur.1, 105, 0.2),
                                blend(cur.2, 110, 0.2),
                            ),
                        );
                    }
                }
            }
        }
        Weather::Windy => {
            let glass_x0 = x + 1;
            let glass_y0 = y + 1;
            let gw = w.saturating_sub(2);
            let gh = h.saturating_sub(2);
            for streak in 0..5u64 {
                let seed = window_idx as u64 * 7 + streak;
                let sx = (seed.wrapping_mul(0x9e37_79b9) % gw as u64) as u16;
                let speed = 50 + (seed.wrapping_mul(0x4f6c_dd1d) % 40);
                let offset = seed.wrapping_mul(0x85eb_ca6b) % (gh as u64).max(1);
                let phase = (elapsed_ms / speed + offset) % gh as u64;
                let len = 3 + (seed % 2) as u16;
                for dy in 0..len {
                    let drift = dy / 2;
                    let px = glass_x0 + (sx + drift) % gw;
                    let py = glass_y0 + ((phase as u16 + dy) % gh);
                    if px < buf.width && py < buf.height {
                        let alpha = 0.35 - (dy as f32 / len as f32) * 0.15;
                        let cur = buf.get(px, py);
                        buf.put(
                            px,
                            py,
                            Rgb(
                                blend(cur.0, 210, alpha),
                                blend(cur.1, 220, alpha),
                                blend(cur.2, 240, alpha),
                            ),
                        );
                    }
                }
            }
        }
        Weather::Clear => {}
    }

    let raw_sunset = sunset_strength(now);
    let twilight_now = {
        use chrono::Timelike;
        let unix_now = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let local = chrono::DateTime::<chrono::Local>::from(std::time::UNIX_EPOCH + unix_now);
        let hf = local.hour() as f32 + local.minute() as f32 / 60.0;
        super::palette::bell(hf, 6.5, 1.5).max(super::palette::bell(hf, 18.5, 1.5))
    };
    let sunset = (raw_sunset * (1.0 - twilight_now * 0.8)).max(0.0);
    if sunset > 0.05 {
        let min_building_h = (glass_h / 5).max(3);
        for dy in 1..h.saturating_sub(1) {
            let glass_dy = dy.saturating_sub(1);
            if glass_dy >= glass_h.saturating_sub(min_building_h) {
                continue;
            }
            for dx in 1..w.saturating_sub(1) {
                let px = x + dx;
                let py = y + dy;
                if px < buf.width && py < buf.height {
                    let cur = buf.get(px, py);
                    let s = sunset * 0.35;
                    buf.put(
                        px,
                        py,
                        Rgb(
                            blend(cur.0, 255, s * 0.4),
                            blend(cur.1, 160, s * 0.25),
                            blend(cur.2, 60, s * 0.1),
                        ),
                    );
                }
            }
        }
    }
}

/// Neon sign panel — dark background with colored glow border, painted in
/// the wall band. The ratatui text widget renders on top with bright colors.
pub(super) fn paint_neon_panel(
    buf: &mut RgbBuffer,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    now: SystemTime,
    theme: &Theme,
) {
    let elapsed_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let pulse = 0.7 + 0.3 * ((elapsed_ms as f32 / 1200.0).sin() * 0.5 + 0.5);

    let panel_bg = theme.office.neon_panel_bg;
    let base = theme.office.neon_frame_base;

    let clamp = |v: f32| v.clamp(0.0, 255.0) as u8;
    let frame_color = Rgb(
        clamp(base.0 as f32 + 25.0 * pulse),
        clamp(base.1 as f32 + 50.0 * pulse),
        clamp(base.2 as f32 + 50.0 * pulse),
    );

    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px >= buf.width || py >= buf.height {
                continue;
            }
            let on_border = dx == 0 || dx == w - 1 || dy == 0 || dy == h - 1;
            if on_border {
                buf.put(px, py, frame_color);
            } else {
                buf.put(px, py, panel_bg);
            }
        }
    }
}

/// Live wall clock — reads system local time and renders hour + minute hands.
/// 5x5 clock face. Hands quantize to 8 cardinal/intercardinal directions
/// (the most a 5x5 sprite can express).
pub(super) fn paint_clock(buf: &mut RgbBuffer, x: u16, y: u16, now: SystemTime, theme: &Theme) {
    let rim = theme.office.clock_rim;
    let face = theme.office.clock_face;
    let hand_color = theme.office.clock_hand;
    let hand_min = hand_color;

    // Face + rim background.
    let bg: &[(u16, u16, Rgb)] = &[
        (1, 0, rim),
        (2, 0, rim),
        (3, 0, rim),
        (0, 1, rim),
        (1, 1, face),
        (2, 1, face),
        (3, 1, face),
        (4, 1, rim),
        (0, 2, rim),
        (1, 2, face),
        (2, 2, face),
        (3, 2, face),
        (4, 2, rim),
        (0, 3, rim),
        (1, 3, face),
        (2, 3, face),
        (3, 3, face),
        (4, 3, rim),
        (1, 4, rim),
        (2, 4, rim),
        (3, 4, rim),
    ];
    for (dx, dy, c) in bg {
        let px = x + dx;
        let py = y + dy;
        if px < buf.width && py < buf.height {
            buf.put(px, py, *c);
        }
    }

    // Decompose `now` into local hour + minute via chrono.
    let unix_now = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let local = chrono::DateTime::<chrono::Local>::from(std::time::UNIX_EPOCH + unix_now);
    use chrono::Timelike;
    let hour = local.hour() % 12;
    let minute = local.minute();

    // Fractional positions around the clock (0.0 = 12 o'clock, 0.25 = 3 o'clock).
    let hour_turns = (hour as f32 + minute as f32 / 60.0) / 12.0;
    let min_turns = minute as f32 / 60.0;

    let put = |buf: &mut RgbBuffer, ox: i32, oy: i32, color: Rgb| {
        let px = x as i32 + 2 + ox;
        let py = y as i32 + 2 + oy;
        if px >= 0 && py >= 0 && (px as u16) < buf.width && (py as u16) < buf.height {
            buf.put(px as u16, py as u16, color);
        }
    };

    // Center pin (always painted).
    put(buf, 0, 0, hand_color);

    // Hour hand: 1 px from center along quantized angle.
    let (hdx, hdy) = octant_offset(hour_turns);
    put(buf, hdx, hdy, hand_color);

    // Minute hand: 2 px from center (longer than hour hand) along its angle.
    let (mdx, mdy) = octant_offset(min_turns);
    put(buf, mdx, mdy, hand_min);
    // (Don't put a 2nd pixel if it falls off the 5x5 — the rim handles it.)
}

/// Quantize a fractional turn (0.0..1.0, 0.0 = north) to one of 8 octant
/// (dx, dy) unit offsets.
fn octant_offset(turn: f32) -> (i32, i32) {
    let oct = ((turn * 8.0).round() as i32).rem_euclid(8);
    match oct {
        0 => (0, -1),
        1 => (1, -1),
        2 => (1, 0),
        3 => (1, 1),
        4 => (0, 1),
        5 => (-1, 1),
        6 => (-1, 0),
        7 => (-1, -1),
        _ => (0, 0),
    }
}

/// Office corridor runner — a darker wood strip with subtle lighter stripes,
/// painted along the walkway band so the eye traces a path connecting the
/// door, meeting room, pantry, cubicles, and lounge. Just texture over the
/// floor — walls and decor paint on top.
pub(super) fn paint_corridor_runner(
    buf: &mut RgbBuffer,
    rect: crate::tui::layout::Bounds,
    theme: &Theme,
) {
    let runner_base = theme.office.runner_base;
    let runner_stripe = theme.office.runner_stripe;
    let runner_edge = theme.office.runner_edge;
    let max_x = (rect.x + rect.width).min(buf.width);
    let max_y = (rect.y + rect.height).min(buf.height);
    for y in rect.y..max_y {
        for x in rect.x..max_x {
            let is_edge = y == rect.y || y + 1 == max_y;
            let is_inner_edge = y == rect.y + 1 || y + 2 == max_y;
            let dy = (y - rect.y) as i32;
            let dx = (x - rect.x) as i32;
            let diamond = ((dx + dy) % 6 == 0) || ((dx - dy).rem_euclid(6) == 0);
            let color = if is_edge {
                runner_edge
            } else if is_inner_edge || diamond {
                runner_stripe
            } else {
                runner_base
            };
            buf.put(x, y, color);
        }
    }
}

/// Elliptical drop-shadow blended toward black at the floor level.
/// Grounds floating sprites so they look like they're standing/sitting
/// on the floor instead of hovering in mid-air. `strength` 0..1 controls
/// the darken amount at the center; falls off quadratically to 0 at edge.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_shadow(
    buf: &mut RgbBuffer,
    cx: u16,
    cy: u16,
    half_w: u16,
    half_h: u16,
    strength: f32,
    theme: &Theme,
) {
    let shadow = theme.office.shadow;
    if half_w == 0 || half_h == 0 || strength <= 0.0 {
        return;
    }
    let min_x = cx.saturating_sub(half_w);
    let max_x = (cx + half_w).min(buf.width);
    let min_y = cy.saturating_sub(half_h);
    let max_y = (cy + half_h).min(buf.height);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let nx = (x as f32 - cx as f32) / half_w as f32;
            let ny = (y as f32 - cy as f32) / half_h as f32;
            let r2 = nx * nx + ny * ny;
            if r2 > 1.0 {
                continue;
            }
            let t = (1.0 - r2) * strength;
            let cur = buf.get(x, y);
            buf.put(
                x,
                y,
                Rgb(
                    blend(cur.0, shadow.0, t),
                    blend(cur.1, shadow.1, t),
                    blend(cur.2, shadow.2, t),
                ),
            );
        }
    }
}
