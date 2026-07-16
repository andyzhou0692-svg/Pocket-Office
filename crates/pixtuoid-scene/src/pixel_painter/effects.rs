use std::time::SystemTime;

use crate::layout::WALKING_Y_OFF;
use pixtuoid_core::sprite::{Rgb, RgbBuffer};

use super::epoch_ms;
use super::palette::blend_rgb;
use crate::layout::Point;
use crate::theme::Theme;

pub(super) fn paint_screen_glow(
    buf: &mut RgbBuffer,
    desk_x: u16,
    desk_y: u16,
    now: SystemTime,
    tint: Rgb,
    theme: &Theme,
) {
    let frame_lit = theme.effects.monitor_frame_lit;
    let glow = tint;
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let glow_bright = blend_rgb(tint, white, 0.4);
    let scanline = blend_rgb(tint, white, 0.7);
    let put = |buf: &mut RgbBuffer, dx: u16, dy: u16, c: Rgb| {
        let px = desk_x + dx;
        let py = desk_y + dy;
        if px < buf.width() && py < buf.height() {
            buf.put(px, py, c);
        }
    };
    const LEFT_SCREEN: std::ops::RangeInclusive<u16> = 3..=4;
    const RIGHT_SCREEN: std::ops::RangeInclusive<u16> = 13..=14;
    for dx in LEFT_SCREEN.clone().chain(RIGHT_SCREEN.clone()) {
        put(buf, dx, 1, glow_bright);
        put(buf, dx, 2, glow);
    }
    // Screen scanline advances one column per this interval.
    const SCANLINE_STEP_MS: u64 = 120;
    let elapsed_ms = epoch_ms(now);
    let phase = (elapsed_ms / SCANLINE_STEP_MS) as u16 + desk_x;
    let local_col = phase % 2;
    for scan_col in [3 + local_col, 13 + local_col] {
        put(buf, scan_col, 1, frame_lit);
        put(buf, scan_col, 2, scanline);
    }
}

pub(super) fn paint_sleep_z(
    buf: &mut RgbBuffer,
    head_anchor: Point,
    now: SystemTime,
    seed: u64,
    theme: &Theme,
) {
    let z_color = theme.effects.sleep_z;
    // One z drifts up from just above the head — brightest at the head, fading
    // to nothing as it climbs. The height-coupled fade (`1.0 - t`) is what keeps
    // it from reading as a solid mark parked over the sprite: it's only briefly
    // visible near the head, then dissolves. RISE_MS is the visible rise+fade
    // span; a short REST_MS gap separates one z from the next.
    const RISE_MS: u64 = 2000;
    const REST_MS: u64 = 400;
    const CYCLE_MS: u64 = RISE_MS + REST_MS;
    const MAX_RISE: u16 = 4;
    const FADE_IN_MS: f32 = 150.0;
    const PEAK_ALPHA: f32 = 0.9;
    let phase_ms = epoch_ms(now).wrapping_add(seed % CYCLE_MS) % CYCLE_MS;
    if phase_ms >= RISE_MS {
        return;
    }
    let t = phase_ms as f32 / RISE_MS as f32;
    // Quick ramp-in over the first FADE_IN_MS avoids a hard pop when a fresh z
    // spawns at the head; the `1.0 - t` term then fades it out as it rises.
    let fade_in = (phase_ms as f32 / FADE_IN_MS).min(1.0);
    let alpha = PEAK_ALPHA * fade_in * (1.0 - t);
    if alpha < 0.06 {
        return;
    }
    let rise = (t * MAX_RISE as f32) as u16;
    let z_x = head_anchor.x + 7;
    let z_y = head_anchor.y.saturating_sub(rise + 3);
    const GLYPH: &[(u16, u16)] = &[(0, 0), (1, 0), (1, 1), (0, 2), (1, 2)];
    for (dx, dy) in GLYPH {
        let px = z_x + dx;
        let py = z_y + dy;
        if px < buf.width() && py < buf.height() {
            let cur = buf.get(px, py);
            buf.put(px, py, blend_rgb(cur, z_color, alpha));
        }
    }
}

pub(super) fn paint_coffee_steam(buf: &mut RgbBuffer, base: Point, now: SystemTime, theme: &Theme) {
    let steam = theme.effects.coffee_steam;
    // Each steam plume fades over one full cycle; 3 plumes staggered by cycle/3.
    const STEAM_CYCLE_MS: u64 = 1800;
    let elapsed_ms = epoch_ms(now);
    for offset in 0..3u64 {
        let phase = (elapsed_ms + offset * (STEAM_CYCLE_MS / 3)) % STEAM_CYCLE_MS;
        let rise = (phase / 140) as u16;
        let alpha = 1.0 - phase as f32 / STEAM_CYCLE_MS as f32;
        if alpha < 0.15 {
            continue;
        }
        let wiggle = if (phase / 200).is_multiple_of(2) {
            0
        } else {
            1
        };
        let px = base.x + wiggle;
        let py = base.y.saturating_sub(rise + 2);
        if px < buf.width() && py < buf.height() {
            let cur = buf.get(px, py);
            buf.put(px, py, blend_rgb(cur, steam, alpha * 0.55));
        }
    }
}

pub(super) fn paint_walking_dust(
    buf: &mut RgbBuffer,
    walker_anchor: Point,
    frame_idx: usize,
    theme: &Theme,
) {
    let dust = theme.effects.walking_dust;
    let foot_y = walker_anchor.y + WALKING_Y_OFF;
    let foot_x = walker_anchor.x + if frame_idx == 0 { 10 } else { 2 };
    if foot_x < buf.width() && foot_y < buf.height() {
        let cur = buf.get(foot_x, foot_y);
        buf.put(foot_x, foot_y, blend_rgb(cur, dust, 0.45));
    }
}

/// Floating heart particles for the "pet the cat" interaction.
/// 4 hearts, staggered 150ms apart, each rising 6px over 1550ms and
/// fading via alpha blend toward the background. Last heart starts at
/// 450ms so all 4 complete within PET_DURATION_MS (2000ms).
pub(super) fn paint_pet_hearts(buf: &mut RgbBuffer, cat_pos: Point, elapsed_ms: u64) {
    const STAGGER_MS: u64 = 150;
    const HEART_LIFE_MS: u64 = 1550;
    let heart_color = Rgb {
        r: 255,
        g: 100,
        b: 100,
    };
    for i in 0..4u64 {
        let stagger = i * STAGGER_MS;
        if elapsed_ms < stagger {
            continue;
        }
        let local_ms = elapsed_ms - stagger;
        if local_ms >= HEART_LIFE_MS {
            continue;
        }
        let t = local_ms as f32 / HEART_LIFE_MS as f32;
        let rise = (t * 6.0) as u16;
        let alpha = 1.0 - t;
        if alpha < 0.05 {
            continue;
        }
        // Spread hearts horizontally: offsets -3, -1, +1, +3
        let dx: i16 = (i as i16) * 2 - 3;
        let hx = (cat_pos.x as i32 + dx as i32).max(0) as u16;
        let hy = cat_pos.y.saturating_sub(4 + rise);
        // 2x2 pixel heart
        for dy in 0..2u16 {
            for ddx in 0..2u16 {
                let px = hx + ddx;
                let py = hy + dy;
                if px < buf.width() && py < buf.height() {
                    let cur = buf.get(px, py);
                    buf.put(px, py, blend_rgb(cur, heart_color, alpha * 0.8));
                }
            }
        }
    }
}

pub(super) fn paint_waiting_bubble(buf: &mut RgbBuffer, anchor: Point, theme: &Theme) {
    let fg = theme.effects.waiting_bubble;
    const GLYPH: &[&[u8]] = &[b".YYY.", b"...Y.", b"..Y..", b"..Y.."];
    let bx = anchor.x + 2;
    let by = anchor.y.saturating_sub(5) & !1u16;
    for (dy, row) in GLYPH.iter().enumerate() {
        for (dx, byte) in row.iter().enumerate() {
            if *byte != b'Y' {
                continue;
            }
            let px = bx + dx as u16;
            let py = by + dy as u16;
            if px < buf.width() && py < buf.height() {
                buf.put(px, py, fg);
            }
        }
    }
}

const LIQUOR_BOTTLE_CAP: Rgb = Rgb {
    r: 48,
    g: 38,
    b: 30,
};
const LIQUOR_BOTTLE_GLASS: Rgb = Rgb {
    r: 112,
    g: 62,
    b: 18,
};
const LIQUOR_BOTTLE_AMBER: Rgb = Rgb {
    r: 204,
    g: 124,
    b: 34,
};
const LIQUOR_BOTTLE_LABEL: Rgb = Rgb {
    r: 238,
    g: 218,
    b: 170,
};

pub(super) fn paint_liquor_bottle(buf: &mut RgbBuffer, anchor: Point) {
    const PIXELS: &[(u16, u16, Rgb)] = &[
        (13, 5, LIQUOR_BOTTLE_CAP),
        (13, 6, LIQUOR_BOTTLE_GLASS),
        (14, 6, LIQUOR_BOTTLE_GLASS),
        (13, 7, LIQUOR_BOTTLE_GLASS),
        (14, 7, LIQUOR_BOTTLE_AMBER),
        (13, 8, LIQUOR_BOTTLE_LABEL),
        (14, 8, LIQUOR_BOTTLE_LABEL),
        (13, 9, LIQUOR_BOTTLE_GLASS),
        (14, 9, LIQUOR_BOTTLE_AMBER),
    ];
    for &(dx, dy, color) in PIXELS {
        let px = anchor.x + dx;
        let py = anchor.y + dy;
        if px < buf.width() && py < buf.height() {
            buf.put(px, py, color);
        }
    }
}

const VAPE_BODY: Rgb = Rgb {
    r: 42,
    g: 48,
    b: 58,
};
const VAPE_TIP: Rgb = Rgb {
    r: 196,
    g: 211,
    b: 218,
};
const VAPE_MIST: Rgb = Rgb {
    r: 245,
    g: 248,
    b: 250,
};

pub(super) fn paint_vape(buf: &mut RgbBuffer, anchor: Point) {
    for (dx, color) in [(12, VAPE_BODY), (13, VAPE_BODY), (14, VAPE_TIP)] {
        let px = anchor.x + dx;
        let py = anchor.y + 6;
        if px < buf.width() && py < buf.height() {
            buf.put(px, py, color);
        }
    }
}

pub(super) fn paint_vape_cloud(buf: &mut RgbBuffer, anchor: Point, elapsed_ms: u64) {
    const CLOUD_LIFE_MS: u64 = 2_000;
    if elapsed_ms >= CLOUD_LIFE_MS {
        return;
    }
    let t = elapsed_ms as f32 / CLOUD_LIFE_MS as f32;
    let fade_in = (elapsed_ms as f32 / 120.0).min(1.0);
    let alpha_base = fade_in * (1.0 - t) * 0.85;
    if alpha_base < 0.04 {
        return;
    }
    const PARTICLES: &[(u16, i16, f32)] = &[
        (1, 0, 1.00),
        (2, -1, 0.92),
        (3, 1, 0.88),
        (4, -2, 0.78),
        (5, 0, 0.82),
        (6, 2, 0.68),
        (7, -1, 0.62),
        (8, 1, 0.56),
    ];
    for &(dx, dy, weight) in PARTICLES {
        let spread_x = (dx as f32 * (0.7 + t * 1.8)).ceil() as u16 + (t * 4.0) as u16;
        let spread_y = (dy as f32 * (0.75 + t * 1.25)).round() as i16;
        let center_x = anchor.x + 14 + spread_x;
        let Some(center_y) = (anchor.y + 6).checked_add_signed(spread_y) else {
            continue;
        };
        for brush_y in -1_i16..=1 {
            for brush_x in -1_i16..=1 {
                let Some(px) = center_x.checked_add_signed(brush_x) else {
                    continue;
                };
                let Some(py) = center_y.checked_add_signed(brush_y) else {
                    continue;
                };
                if px < buf.width() && py < buf.height() {
                    let edge_weight = if brush_x == 0 && brush_y == 0 {
                        1.0
                    } else {
                        0.72
                    };
                    let cur = buf.get(px, py);
                    buf.put(
                        px,
                        py,
                        blend_rgb(cur, VAPE_MIST, alpha_base * weight * edge_weight),
                    );
                }
            }
        }
    }
}

pub(super) fn paint_suspicious_glance(
    buf: &mut RgbBuffer,
    anchor: Point,
    habit: crate::habits::CharacterHabit,
) {
    use crate::habits::CharacterHabit;
    if !matches!(habit, CharacterHabit::LookLeft | CharacterHabit::LookRight)
        || anchor.x + 11 >= buf.width()
        || anchor.y + 4 >= buf.height()
    {
        return;
    }
    let eye = buf.get(anchor.x + 5, anchor.y + 4);
    let skin = buf.get(anchor.x + 6, anchor.y + 4);
    for x in [5, 10] {
        buf.put(anchor.x + x, anchor.y + 4, skin);
    }
    let shifted = match habit {
        CharacterHabit::LookLeft => [4, 9],
        CharacterHabit::LookRight => [6, 11],
        _ => return,
    };
    for x in shifted {
        buf.put(anchor.x + x, anchor.y + 4, eye);
    }
}

/// The Top-tier flame crown (`burn::BurnTier::Top`) — a 2-frame flicker above
/// the sprite's hair, painted AFTER the character blit so it rides every pose
/// (seated/walking/standing) through the one `paint_character_at` seam. The
/// aesthetic is the user-ratified mockup (2026-07-10): tips capped ≤2 px above
/// the hair top so the flame never collides with the name-badge row; the
/// asymmetric two-frame flicker is what reads as fire, not a hat. INTEGER
/// phase division before any float (the epoch-ms-as-f32 freeze sharp edge).
/// The flame gradient's deep-ember base — ONE literal shared with the
/// Premium ember-hair recolor (`palette::agent_palette`), so a gradient
/// tweak can't desync the hair from the crown.
pub(crate) const FLAME_DEEP: Rgb = Rgb {
    r: 0xc2,
    g: 0x28,
    b: 0x12,
};

/// The flame gradient's yellow tip. `pub(crate)` alongside [`FLAME_DEEP`] so
/// render tests assert the REAL painted colors instead of re-hardcoding them.
pub(crate) const FLAME_TIP: Rgb = Rgb {
    r: 0xff,
    g: 0xd2,
    b: 0x4a,
};

pub(super) fn paint_flame_crown(
    buf: &mut RgbBuffer,
    anchor: Point,
    sprite_w: u16,
    now: SystemTime,
) {
    // Ratified flame palette (deep ember → orange → yellow tip → hot core);
    // the deep base is the shared FLAME_DEEP (also the Premium hair recolor).
    const MID: Rgb = Rgb {
        r: 0xe8,
        g: 0x64,
        b: 0x1f,
    };
    const TIP: Rgb = FLAME_TIP;
    const CORE: Rgb = Rgb {
        r: 0xff,
        g: 0xf3,
        b: 0xa0,
    };
    const FLICKER_MS: u64 = 260;
    let f2 = (epoch_ms(now) / FLICKER_MS) % 2 == 1;

    // Head-center column; the crown hugs the hair's top row (anchor.y) and
    // rises two rows above it. Pattern is (dx from center-left, dy up, color).
    let cx = anchor.x + sprite_w / 2;
    let frame_a: &[(i32, u16, Rgb)] = &[
        // crown row over the hair top
        (-2, 0, MID),
        (-1, 0, MID),
        (0, 0, FLAME_DEEP),
        (1, 0, MID),
        // first rise
        (-2, 1, MID),
        (-1, 1, CORE),
        (0, 1, MID),
        (1, 1, TIP),
        // tips
        (-2, 2, TIP),
        (0, 2, TIP),
    ];
    let frame_b: &[(i32, u16, Rgb)] = &[
        (-2, 0, MID),
        (-1, 0, FLAME_DEEP),
        (0, 0, MID),
        (1, 0, MID),
        (-2, 1, TIP),
        (-1, 1, MID),
        (0, 1, CORE),
        (1, 1, MID),
        (-1, 2, TIP),
        (1, 2, TIP),
    ];
    for &(dx, dy, c) in if f2 { frame_b } else { frame_a } {
        let Some(px) = cx.checked_add_signed(dx as i16) else {
            continue;
        };
        let Some(py) = anchor.y.checked_sub(dy) else {
            continue;
        };
        if px < buf.width() && py < buf.height() {
            buf.put(px, py, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn theme() -> &'static Theme {
        crate::theme::theme_by_name("normal").expect("normal theme")
    }

    #[test]
    fn active_glow_lights_two_monitor_interiors_without_repainting_the_table() {
        let black = Rgb { r: 0, g: 0, b: 0 };
        let tint = Rgb {
            r: 40,
            g: 180,
            b: 220,
        };
        let mut buf = RgbBuffer::filled(24, 16, black);

        paint_screen_glow(&mut buf, 2, 3, SystemTime::UNIX_EPOCH, tint, theme());

        for y in 4..=5 {
            assert_ne!(buf.get(5, y), black, "left monitor is lit");
            assert_ne!(buf.get(15, y), black, "right monitor is lit");
            assert_eq!(buf.get(4, y), black, "left bezel stays visible");
            assert_eq!(buf.get(14, y), black, "right bezel stays visible");
            assert_eq!(buf.get(10, y), black, "center gap stays open");
        }
        for x in 2..20 {
            assert_eq!(
                buf.get(x, 6),
                black,
                "active screens must not become three-row blue blocks"
            );
        }
        for x in 2..20 {
            assert_eq!(
                buf.get(x, 7),
                black,
                "the glow must not repaint the tabletop or apron"
            );
        }
    }

    #[test]
    fn alison_vape_paints_a_small_device_beside_the_mouth() {
        let background = Rgb { r: 1, g: 2, b: 3 };
        let anchor = Point { x: 8, y: 8 };
        let mut buf = RgbBuffer::filled(40, 32, background);

        paint_vape(&mut buf, anchor);

        assert_ne!(buf.get(anchor.x + 12, anchor.y + 6), background);
        assert_ne!(buf.get(anchor.x + 13, anchor.y + 6), background);
        assert_ne!(buf.get(anchor.x + 14, anchor.y + 6), background);
    }

    #[test]
    fn alison_vape_cloud_expands_from_the_face_and_is_gone_at_two_seconds() {
        let background = Rgb { r: 1, g: 2, b: 3 };
        let anchor = Point { x: 8, y: 8 };
        let render = |elapsed_ms| {
            let mut buf = RgbBuffer::filled(48, 32, background);
            paint_vape_cloud(&mut buf, anchor, elapsed_ms);
            buf
        };

        let early = render(250);
        assert_ne!(early.get(anchor.x + 15, anchor.y + 6), background);

        let expanded = render(1_000);
        let expanded_pixels = expanded
            .as_slice()
            .iter()
            .filter(|pixel| **pixel != background)
            .count();
        assert!(
            expanded_pixels >= 40,
            "the dramatic cloud should cover roughly five times the original eight-pixel footprint; got {expanded_pixels} pixels"
        );
        assert!(
            (anchor.x + 18..=anchor.x + 23).any(|x| {
                (anchor.y + 3..=anchor.y + 9).any(|y| expanded.get(x, y) != background)
            }),
            "the mid-exhale cloud should spread outward into a cone"
        );

        let finished = render(2_000);
        assert_eq!(finished.as_slice(), render(2_500).as_slice());
        assert!(finished.as_slice().iter().all(|pixel| *pixel == background));
    }

    fn render(head: Point, phase_ms: u64) -> RgbBuffer {
        let mut buf = RgbBuffer::filled(64, 64, Rgb { r: 0, g: 0, b: 0 });
        let now = SystemTime::UNIX_EPOCH + Duration::from_millis(phase_ms);
        paint_sleep_z(&mut buf, head, now, 0, theme());
        buf
    }

    fn lum(c: Rgb) -> u32 {
        c.r as u32 + c.g as u32 + c.b as u32
    }

    // Topmost lit pixel in the z's column, if any (kept independent of MAX_RISE).
    fn top_lit(buf: &RgbBuffer, head: Point, bg: Rgb) -> Option<(u16, Rgb)> {
        let zx = head.x + 7;
        (0..head.y).find_map(|y| {
            let p = buf.get(zx, y);
            (p != bg).then_some((y, p))
        })
    }

    #[test]
    fn sleep_z_dims_as_it_rises_then_rests() {
        let head = Point { x: 20, y: 30 };
        let bg = Rgb { r: 0, g: 0, b: 0 };
        let zx = head.x + 7;

        // Just spawned (rise 0 for any MAX_RISE): brightest, at the spawn row.
        let low = render(head, 200);
        let low_px = low.get(zx, head.y - 3);
        assert!(lum(low_px) > 0, "z near the head is visible");

        // Later it has risen AND faded ("higher = blurrier").
        let high = render(head, 1600);
        let (top_y, top_px) = top_lit(&high, head, bg).expect("risen z still visible");
        assert!(top_y < head.y - 3, "z rose above its spawn row");
        assert!(
            lum(top_px) < lum(low_px),
            "a higher z must be dimmer than one at the head"
        );

        // During the rest gap (phase >= RISE_MS) nothing is painted at all.
        let resting = render(head, 2300);
        for y in 0..resting.height() {
            for x in 0..resting.width() {
                assert_eq!(resting.get(x, y), bg, "no z during the rest gap");
            }
        }
    }

    #[test]
    fn liquor_bottle_reads_as_a_raised_amber_bottle_beside_the_face() {
        let bg = Rgb { r: 1, g: 2, b: 3 };
        let anchor = Point { x: 8, y: 8 };
        let mut buf = RgbBuffer::filled(32, 32, bg);

        paint_liquor_bottle(&mut buf, anchor);

        assert_eq!(buf.get(anchor.x + 13, anchor.y + 5), LIQUOR_BOTTLE_CAP);
        assert_eq!(buf.get(anchor.x + 13, anchor.y + 6), LIQUOR_BOTTLE_GLASS);
        assert_eq!(buf.get(anchor.x + 14, anchor.y + 7), LIQUOR_BOTTLE_AMBER);
        assert_eq!(buf.get(anchor.x + 13, anchor.y + 8), LIQUOR_BOTTLE_LABEL);
        assert_eq!(buf.get(anchor.x + 12, anchor.y + 9), bg);
    }
}
