use std::time::SystemTime;

use ascii_agents_core::sprite::{Rgb, RgbBuffer};

use super::palette::blend;
use crate::tui::layout::Point;
use crate::tui::theme::Theme;

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
    let glow_bright = Rgb(
        blend(tint.0, 255, 0.4),
        blend(tint.1, 255, 0.4),
        blend(tint.2, 255, 0.4),
    );
    let scanline = Rgb(
        blend(tint.0, 255, 0.7),
        blend(tint.1, 255, 0.7),
        blend(tint.2, 255, 0.7),
    );
    let put = |buf: &mut RgbBuffer, dx: u16, dy: u16, c: Rgb| {
        let px = desk_x + dx;
        let py = desk_y + dy;
        if px < buf.width && py < buf.height {
            buf.put(px, py, c);
        }
    };
    for dx in 3..=10 {
        put(buf, dx, 0, frame_lit);
    }
    for dx in 4..=9 {
        put(buf, dx, 1, glow_bright);
        put(buf, dx, 2, glow);
    }
    for dx in 4..=9 {
        put(buf, dx, 3, frame_lit);
    }
    let elapsed_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let phase = (elapsed_ms / 120) as u16 + desk_x;
    let scan_col = 4 + (phase % 6);
    put(buf, scan_col, 1, scanline);
    put(buf, scan_col, 2, scanline);
}

pub(super) fn paint_sleep_z(
    buf: &mut RgbBuffer,
    head_anchor: Point,
    now: SystemTime,
    seed: u64,
    theme: &Theme,
) {
    let z_color = theme.effects.sleep_z;
    const CYCLE_MS: u64 = 2400;
    let elapsed_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let phase_ms = elapsed_ms.wrapping_add(seed % CYCLE_MS) % CYCLE_MS;
    if phase_ms >= CYCLE_MS - 400 {
        return;
    }
    let rise = (phase_ms / 180) as u16;
    let z_x = head_anchor.x + 5;
    let z_y = head_anchor.y.saturating_sub(rise + 3);
    let pixels: &[(u16, u16)] = &[(0, 0), (1, 0), (1, 1), (0, 2), (1, 2)];
    for (dx, dy) in pixels {
        let px = z_x + dx;
        let py = z_y + dy;
        if px < buf.width && py < buf.height {
            buf.put(px, py, z_color);
        }
    }
}

pub(super) fn paint_coffee_steam(buf: &mut RgbBuffer, base: Point, now: SystemTime, theme: &Theme) {
    let steam = theme.effects.coffee_steam;
    let elapsed_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    for offset in 0..3u64 {
        let phase = (elapsed_ms + offset * 600) % 1800;
        let rise = (phase / 140) as u16;
        let alpha = 1.0 - phase as f32 / 1800.0;
        if alpha < 0.15 {
            continue;
        }
        let wiggle = if (phase / 200) % 2 == 0 { 0 } else { 1 };
        let px = base.x + wiggle;
        let py = base.y.saturating_sub(rise + 2);
        if px < buf.width && py < buf.height {
            let cur = buf.get(px, py);
            buf.put(
                px,
                py,
                Rgb(
                    blend(cur.0, steam.0, alpha * 0.55),
                    blend(cur.1, steam.1, alpha * 0.55),
                    blend(cur.2, steam.2, alpha * 0.55),
                ),
            );
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
    let foot_y = walker_anchor.y + 12;
    let foot_x = walker_anchor.x + if frame_idx == 0 { 6 } else { 1 };
    if foot_x < buf.width && foot_y < buf.height {
        let cur = buf.get(foot_x, foot_y);
        buf.put(
            foot_x,
            foot_y,
            Rgb(
                blend(cur.0, dust.0, 0.45),
                blend(cur.1, dust.1, 0.45),
                blend(cur.2, dust.2, 0.45),
            ),
        );
    }
}

pub(super) fn paint_waiting_bubble(buf: &mut RgbBuffer, anchor: Point, theme: &Theme) {
    let fg = theme.effects.waiting_bubble;
    const GLYPH: &[&[u8]] = &[
        b".YYYYY..",
        b".YYYYY..",
        b"....YY..",
        b"..YY....",
        b"..YY....",
        b"..YY....",
    ];
    let bx = anchor.x;
    let by = anchor.y.saturating_sub(7) & !1u16;
    for (dy, row) in GLYPH.iter().enumerate() {
        for (dx, byte) in row.iter().enumerate() {
            if *byte != b'Y' {
                continue;
            }
            let px = bx + dx as u16;
            let py = by + dy as u16;
            if px < buf.width && py < buf.height {
                buf.put(px, py, fg);
            }
        }
    }
}
