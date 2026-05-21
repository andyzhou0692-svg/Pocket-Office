//! Top-down coworking-lounge renderer.
//!
//! Zone-based layout via `tui::layout`, state→pose derivation via `tui::pose`.
//! This file owns the actual pixel painting (floor, walls, decor, character
//! sprites, terminal flush). Layout and pose are pure functions tested in
//! isolation; this file is the integrator.

use std::io::{stdout, Stdout};
use std::time::SystemTime;

use anyhow::Result;
use ascii_agents_core::sprite::blit::blit_frame;
use ascii_agents_core::sprite::format::Pack;
use ascii_agents_core::sprite::{Frame, Palette, Pixel, Rgb, RgbBuffer};
use ascii_agents_core::state::ActivityState;
use ascii_agents_core::{AgentSlot, SceneState};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

use crate::tui::layout::{Layout, Point, DESK_H, DESK_W};
use crate::tui::pose::{self, Pose};

pub type Term = Terminal<CrosstermBackend<Stdout>>;

// --- Colors ---------------------------------------------------------------
const BG: Rgb = Rgb(28, 32, 40);
const PLANK_A: Rgb = Rgb(120, 84, 50);
const PLANK_B: Rgb = Rgb(100, 70, 38);
const PLANK_LINE: Rgb = Rgb(72, 48, 24);
const WALL: Rgb = Rgb(56, 56, 70);
const WALL_TRIM: Rgb = Rgb(80, 80, 100);
const BASEBOARD: Rgb = Rgb(40, 40, 52);
const RUG_PALETTE: &[Rgb] = &[
    Rgb(0x4a, 0x55, 0x80),
    Rgb(0x6a, 0x3f, 0x55),
    Rgb(0x40, 0x60, 0x4f),
    Rgb(0x6e, 0x4d, 0x2e),
];
const SHIRT_PRESETS: &[Rgb] = &[
    Rgb(0x2e, 0x62, 0xcf),
    Rgb(0x16, 0xa0, 0x6e),
    Rgb(0xb0, 0x32, 0xa8),
    Rgb(0xc6, 0x6a, 0x1e),
    Rgb(0x6c, 0x4f, 0x9e),
    Rgb(0x9c, 0x27, 0x27),
    Rgb(0x32, 0x82, 0x9b),
    Rgb(0x80, 0x55, 0x32),
];
const HAIR_PRESETS: &[Rgb] = &[
    Rgb(0x2a, 0x1a, 0x0e),
    Rgb(0x52, 0x32, 0x10),
    Rgb(0xc7, 0xa3, 0x4a),
    Rgb(0x7a, 0x32, 0x10),
    Rgb(0x3a, 0x3a, 0x3a),
];

// --- Terminal lifecycle ---------------------------------------------------
pub fn setup_terminal() -> Result<Term> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

pub fn teardown_terminal(term: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    Ok(())
}

// --- Per-agent recolor ----------------------------------------------------
fn agent_palette(base: &Palette, agent: &AgentSlot) -> Palette {
    let seed = agent.agent_id.raw() as usize;
    let shirt = SHIRT_PRESETS[seed % SHIRT_PRESETS.len()];
    let hair = HAIR_PRESETS[(seed / 7) % HAIR_PRESETS.len()];
    base.with_override('B', Some(shirt))
        .with_override('H', Some(hair))
}

fn recolor_frame(frame: &Frame, pal: &Palette, base_pal: &Palette) -> Frame {
    let base_shirt = base_pal.get('B').flatten();
    let base_hair = base_pal.get('H').flatten();
    let agent_shirt = pal.get('B').flatten();
    let agent_hair = pal.get('H').flatten();
    let pixels: Vec<Pixel> = frame
        .pixels
        .iter()
        .map(|p| match p {
            Some(rgb) if Some(*rgb) == base_shirt => agent_shirt,
            Some(rgb) if Some(*rgb) == base_hair => agent_hair,
            other => *other,
        })
        .collect();
    Frame {
        width: frame.width,
        height: frame.height,
        pixels,
    }
}

// --- Floor / walls / decor -----------------------------------------------
fn paint_floor_and_walls(buf: &mut RgbBuffer, buf_w: u16, buf_h: u16) {
    const PLANK_H: u16 = 6;
    const TOP_WALL_H: u16 = 6;
    const BASEBOARD_H: u16 = 3;

    for y in 0..buf_h {
        let band = y / PLANK_H;
        let seam_offset = (band as u32 * 13) % 16;
        for x in 0..buf_w {
            let in_seam = y % PLANK_H == PLANK_H - 1
                || ((x as u32).wrapping_add(seam_offset)) % 16 == 0;
            let color = if in_seam {
                PLANK_LINE
            } else if band % 2 == 0 {
                PLANK_A
            } else {
                PLANK_B
            };
            buf.put(x, y, color);
        }
    }
    for y in 0..TOP_WALL_H.min(buf_h) {
        for x in 0..buf_w {
            buf.put(x, y, WALL);
        }
    }
    if TOP_WALL_H < buf_h {
        for x in 0..buf_w {
            buf.put(x, TOP_WALL_H, WALL_TRIM);
        }
    }
    let base_y = buf_h.saturating_sub(BASEBOARD_H);
    for y in base_y..buf_h {
        for x in 0..buf_w {
            buf.put(x, y, BASEBOARD);
        }
    }
}

fn paint_rug(buf: &mut RgbBuffer, x: u16, y: u16, w: u16, h: u16, color: Rgb) {
    let lighter = Rgb(
        color.0.saturating_add(40),
        color.1.saturating_add(40),
        color.2.saturating_add(40),
    );
    for dy in 1..h.saturating_sub(1) {
        for dx in 1..w.saturating_sub(1) {
            let px = x + dx;
            let py = y + dy;
            if px >= buf.width || py >= buf.height {
                continue;
            }
            let on_border = dy == 1 || dy + 2 == h || dx == 1 || dx + 2 == w;
            buf.put(px, py, if on_border { lighter } else { color });
        }
    }
}

fn paint_lounge_decor(buf: &mut RgbBuffer, layout: &Layout, pack: &Pack) {
    if let Some(couch) = pack.animation("couch").and_then(|a| a.frames.first()) {
        let cx = layout.lounge_band.x + 2;
        let cy = layout.lounge_band.y + (layout.lounge_band.height.saturating_sub(couch.height)) / 2;
        blit_frame(couch, cx, cy, buf);
    }
    if let Some(coffee) = pack.animation("coffee").and_then(|a| a.frames.first()) {
        let cx = layout
            .lounge_band
            .x
            .saturating_add(layout.lounge_band.width)
            .saturating_sub(coffee.width + 4);
        let cy = layout.lounge_band.y + (layout.lounge_band.height.saturating_sub(coffee.height)) / 2;
        blit_frame(coffee, cx, cy, buf);
    }
    if let Some(plant) = pack.animation("plant").and_then(|a| a.frames.first()) {
        for (i, wp) in layout.waypoints.iter().enumerate() {
            if i == 0 || i == layout.waypoints.len() - 1 {
                continue;
            }
            let px = wp.x.saturating_sub(plant.width / 2);
            let py = wp.y.saturating_sub(plant.height / 2);
            blit_frame(plant, px, py, buf);
        }
    }
}

// --- Character placement --------------------------------------------------
fn seated_anchor(desk: Point) -> Point {
    Point {
        x: desk.x + DESK_W.saturating_sub(8) / 2,
        y: desk.y.saturating_sub(8),
    }
}

fn standing_at_desk_anchor(desk: Point) -> Point {
    Point {
        x: desk.x + DESK_W.saturating_sub(6) / 2,
        y: desk.y.saturating_sub(12),
    }
}

fn walking_anchor(p: Point) -> Point {
    Point {
        x: p.x.saturating_sub(3),
        y: p.y.saturating_sub(12),
    }
}

fn waypoint_anchor(wp: Point) -> Point {
    Point {
        x: wp.x.saturating_sub(3),
        y: wp.y.saturating_sub(12),
    }
}

fn walking_position(from: Point, to: Point, t_x1000: u16) -> Point {
    let t = t_x1000 as i32;
    let dx = to.x as i32 - from.x as i32;
    let dy = to.y as i32 - from.y as i32;
    Point {
        x: (from.x as i32 + dx * t / 1000) as u16,
        y: (from.y as i32 + dy * t / 1000) as u16,
    }
}

/// Paint a character at an arbitrary anchor with per-agent recolor.
fn paint_character_at(
    buf: &mut RgbBuffer,
    anim_name: &str,
    frame_idx: usize,
    anchor: Point,
    agent: &AgentSlot,
    pack: &Pack,
) {
    let base_pal = pack.palette.clone();
    let pal = agent_palette(&base_pal, agent);
    let Some(anim) = pack.animation(anim_name) else { return };
    let Some(frame) = anim.frames.get(frame_idx).or_else(|| anim.frames.first()) else { return };
    let recolored = recolor_frame(frame, &pal, &base_pal);
    blit_frame(&recolored, anchor.x, anchor.y, buf);
}

// --- Speech bubble overlay (kept from the prior renderer) -----------------
fn paint_waiting_bubble(buf: &mut RgbBuffer, anchor: Point) {
    const BUBBLE_FG: Rgb = Rgb(240, 200, 80);
    const BUBBLE_BG: Rgb = Rgb(30, 30, 40);
    let bx = anchor.x;
    let by = anchor.y.saturating_sub(4);
    let dots: &[(u16, u16, Rgb)] = &[
        (0, 0, BUBBLE_BG), (1, 0, BUBBLE_BG), (2, 0, BUBBLE_BG), (3, 0, BUBBLE_BG), (4, 0, BUBBLE_BG),
        (0, 1, BUBBLE_BG), (2, 1, BUBBLE_FG), (4, 1, BUBBLE_BG),
        (0, 2, BUBBLE_BG), (1, 2, BUBBLE_BG), (2, 2, BUBBLE_FG), (3, 2, BUBBLE_BG), (4, 2, BUBBLE_BG),
    ];
    for (dx, dy, c) in dots {
        let px = bx + dx;
        let py = by + dy;
        if px < buf.width && py < buf.height {
            buf.put(px, py, *c);
        }
    }
}

// --- draw_scene ----------------------------------------------------------
pub fn draw_scene<B: Backend>(
    term: &mut Terminal<B>,
    scene: &SceneState,
    pack: &Pack,
    now: SystemTime,
    buf: &mut RgbBuffer,
) -> Result<()> {
    let agents: Vec<_> = scene.agents.values().cloned().collect();
    term.draw(|f| {
        let size = f.area();

        let title = Paragraph::new(Line::from(vec![
            Span::raw(" ascii-agents — "),
            Span::raw(format!(
                "{} session{} ",
                agents.len(),
                if agents.len() == 1 { "" } else { "s" }
            )),
        ]));
        f.render_widget(
            title,
            Rect { x: size.x, y: size.y, width: size.width, height: 1 },
        );

        let footer = Paragraph::new(Span::raw(" [q] quit "))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(
            footer,
            Rect {
                x: size.x,
                y: size.y + size.height.saturating_sub(1),
                width: size.width,
                height: 1,
            },
        );

        let scene_rect = Rect {
            x: size.x,
            y: size.y + 1,
            width: size.width,
            height: size.height.saturating_sub(2),
        };
        if scene_rect.width < 20 || scene_rect.height < 12 {
            return;
        }

        let buf_w = scene_rect.width;
        let buf_h = scene_rect.height * 2;
        buf.ensure_size(buf_w, buf_h, BG);

        let Some(layout) = Layout::compute(buf_w, buf_h, agents.len()) else {
            return;
        };

        paint_floor_and_walls(buf, buf_w, buf_h);
        paint_lounge_decor(buf, &layout, pack);

        // Pass 1: rugs + desks.
        let desk_anim = pack.animation("desk");
        for (i, desk) in layout.home_desks.iter().enumerate() {
            let agent = &agents[i];
            let rug = RUG_PALETTE[(agent.agent_id.raw() as usize / 11) % RUG_PALETTE.len()];
            paint_rug(
                buf,
                desk.x.saturating_sub(1),
                desk.y.saturating_sub(10),
                DESK_W + 2,
                DESK_H + 12,
                rug,
            );
            if let Some(frame) = desk_anim.and_then(|a| a.frames.first()) {
                blit_frame(frame, desk.x, desk.y, buf);
            }
        }

        // Pass 2: characters by pose.
        for (i, desk) in layout.home_desks.iter().enumerate() {
            let agent = &agents[i];
            let Some(p) = pose::derive(agent, now, &layout) else { continue };
            match p {
                Pose::SeatedIdle => {
                    paint_character_at(buf, "seated", 0, seated_anchor(*desk), agent, pack);
                }
                Pose::SeatedTyping { frame } => {
                    paint_character_at(buf, "typing", frame, seated_anchor(*desk), agent, pack);
                }
                Pose::StandingAtDesk => {
                    let anchor = standing_at_desk_anchor(*desk);
                    paint_character_at(buf, "standing", 0, anchor, agent, pack);
                    if matches!(agent.state, ActivityState::Waiting { .. }) {
                        paint_waiting_bubble(buf, anchor);
                    }
                }
                Pose::StandingAtWaypoint { wp } => {
                    if let Some(wp_pt) = layout.waypoints.get(wp) {
                        paint_character_at(
                            buf,
                            "standing",
                            0,
                            waypoint_anchor(*wp_pt),
                            agent,
                            pack,
                        );
                    }
                }
                Pose::Walking { from, to, t_x1000, frame } => {
                    let pos = walking_position(from, to, t_x1000);
                    paint_character_at(buf, "walking", frame, walking_anchor(pos), agent, pack);
                }
            }
        }

        // Flush half-block cells.
        let term_buf = f.buffer_mut();
        let w = buf.width as usize;
        let cell_rows = (buf.height / 2) as usize;
        for cy in 0..cell_rows {
            for cx in 0..(buf.width as usize) {
                let x = scene_rect.x + cx as u16;
                let y = scene_rect.y + cy as u16;
                if x >= scene_rect.x + scene_rect.width
                    || y >= scene_rect.y + scene_rect.height
                {
                    continue;
                }
                let py_top = cy * 2;
                let py_bot = cy * 2 + 1;
                let fg = buf.pixels[py_top * w + cx];
                let bg = buf.pixels[py_bot * w + cx];
                let cell = &mut term_buf[(x, y)];
                cell.set_symbol("▀");
                cell.fg = Color::Rgb(fg.0, fg.1, fg.2);
                cell.bg = Color::Rgb(bg.0, bg.1, bg.2);
            }
        }

        // Labels above each home desk.
        for (i, desk) in layout.home_desks.iter().enumerate() {
            let agent = &agents[i];
            let lx = scene_rect.x + desk.x;
            let ly = scene_rect.y + (desk.y / 2).saturating_sub(1);
            let para = Paragraph::new(Span::styled(
                format!("{} {}", agent.label, summarize_state(&agent.state)),
                Style::default().fg(Color::White),
            ));
            f.render_widget(
                para,
                Rect { x: lx, y: ly, width: DESK_W + 4, height: 1 },
            );
        }
    })?;
    Ok(())
}

fn summarize_state(state: &ActivityState) -> &'static str {
    match state {
        ActivityState::Idle => "idle",
        ActivityState::Active { .. } => "working",
        ActivityState::Waiting { .. } => "waiting",
    }
}
