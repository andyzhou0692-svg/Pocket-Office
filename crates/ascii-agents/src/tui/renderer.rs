//! Terminal-coupled rendering: orchestrator (`draw_scene`), half-block
//! flush, label/tooltip/notice widget overlays, and terminal lifecycle.
//!
//! The pure-pixel pass (floor/walls/decor/characters → `RgbBuffer`) lives
//! in `tui::pixel_painter`. This file is the integrator that calls into
//! that pipeline and then hands the buffer to ratatui.

use std::collections::HashMap;
use std::io::{stdout, Stdout};
use std::time::SystemTime;

use anyhow::Result;
use ascii_agents_core::sprite::format::Pack;
use ascii_agents_core::sprite::{Rgb, RgbBuffer};
use ascii_agents_core::state::ActivityState;
use ascii_agents_core::{AgentId, AgentSlot, SceneState};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

use ascii_agents_core::walkable::OccupancyOverlay;

use crate::tui::frame_cache::FrameCache;
use crate::tui::layout::{Layout, DESK_W};
use crate::tui::pathfind::Router;
use crate::tui::pixel_painter::{character_anchor, render_to_rgb_buffer};
use crate::tui::pose;

fn to_color(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// Persistent scrolling ticker queue. Messages append to the end and scroll
/// off the left naturally — like a news crawl. The queue rebuilds only when
/// the set of active tool details changes, preserving scroll continuity.
pub struct TickerQueue {
    buffer: String,
    last_snapshot: String,
}

impl Default for TickerQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl TickerQueue {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            last_snapshot: String::new(),
        }
    }

    pub fn update(&mut self, scene: &SceneState) {
        let mut items: Vec<String> = scene
            .agents
            .values()
            .filter(|a| a.exiting_at.is_none())
            .filter_map(|a| match &a.state {
                ActivityState::Active { detail, .. } => {
                    let tool = detail.as_deref().unwrap_or("working");
                    Some(format!("{}: {}", a.label, tool))
                }
                ActivityState::Waiting { reason } => Some(format!("{}: ?{}", a.label, reason)),
                _ => None,
            })
            .collect();
        items.sort();
        let snapshot = items.join("|");
        if snapshot != self.last_snapshot {
            self.last_snapshot = snapshot;
            for item in &items {
                self.buffer.push_str(item);
                self.buffer.push_str("  |  ");
            }
            const MAX_CHARS: usize = 512;
            let char_count = self.buffer.chars().count();
            if char_count > MAX_CHARS {
                let trim_chars = char_count - MAX_CHARS;
                if let Some((byte_idx, _)) = self.buffer.char_indices().nth(trim_chars) {
                    self.buffer.drain(..byte_idx);
                }
            }
        }
    }

    pub fn visible(&self, width: usize, now: SystemTime) -> String {
        if self.buffer.is_empty() {
            return String::new();
        }
        let elapsed_ms = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let char_count = self.buffer.chars().count();
        let offset = (elapsed_ms / 150) as usize % char_count;
        let doubled = format!("{}{}", self.buffer, self.buffer);
        doubled.chars().skip(offset).take(width).collect()
    }
}

/// Clip a widget rect to fit inside `bounds`. Returns `None` if the rect
/// falls fully outside or has zero width/height after clipping — callers
/// use that to skip the render entirely. Prevents ratatui's
/// "index outside of buffer" panic when label/notice widgets land near
/// the right or bottom edge.
fn clip_widget_rect(rect: Rect, bounds: Rect) -> Option<Rect> {
    if rect.x >= bounds.x + bounds.width || rect.y >= bounds.y + bounds.height {
        return None;
    }
    if rect.x + rect.width <= bounds.x || rect.y + rect.height <= bounds.y {
        return None;
    }
    let x = rect.x.max(bounds.x);
    let y = rect.y.max(bounds.y);
    let right = (rect.x + rect.width).min(bounds.x + bounds.width);
    let bot = (rect.y + rect.height).min(bounds.y + bounds.height);
    if right <= x || bot <= y {
        return None;
    }
    Some(Rect {
        x,
        y,
        width: right - x,
        height: bot - y,
    })
}

pub type Term = Terminal<CrosstermBackend<Stdout>>;

// --- Terminal lifecycle ---------------------------------------------------
pub fn setup_terminal() -> Result<Term> {
    enable_raw_mode()?;
    let mut out = stdout();
    // EnableMouseCapture turns on the terminal's mouse-event reporting.
    // Modern terminals emit MouseEventKind::Moved on cursor motion (no
    // button required), which is how we drive the hover tooltip.
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

pub fn teardown_terminal(term: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    term.show_cursor()?;
    Ok(())
}

// --- draw_scene ----------------------------------------------------------
//
// `draw_scene` is the orchestrator: get terminal geometry, compute the
// layout, run the pure pixel pass, then flush to the terminal. The two
// helpers below are deliberately split:
//
//   * `render_to_rgb_buffer` — pure RGB output. No ratatui types, no
//     terminal I/O. Can be called by any renderer (web canvas, PNG
//     snapshot, GIF capture).
//   * `flush_to_terminal` — ratatui half-block compression + label overlay
//     + bulletin notice + footer. Terminal-specific, runs inside
//     `term.draw`.
#[allow(clippy::too_many_arguments)]
pub fn draw_scene<B: Backend>(
    term: &mut Terminal<B>,
    scene: &SceneState,
    pack: &Pack,
    now: SystemTime,
    buf: &mut RgbBuffer,
    cache: &mut FrameCache,
    router: &mut dyn Router,
    overlay: &mut OccupancyOverlay,
    history: &mut pose::PoseHistory,
    mouse_pos: Option<(u16, u16)>,
    pinned_agent: Option<AgentId>,
    ticker: &TickerQueue,
    theme: &crate::tui::theme::Theme,
    theme_picker: Option<usize>,
    floor_info: Option<(usize, usize)>,
    floor: crate::tui::floor::FloorMeta,
) -> Result<()> {
    let term_size = term.size()?;
    let full_rect = Rect {
        x: 0,
        y: 0,
        width: term_size.width,
        height: term_size.height,
    };
    let scene_rect = Rect {
        x: 0,
        y: 0,
        width: full_rect.width,
        height: full_rect.height.saturating_sub(1),
    };
    if scene_rect.width < 20 || scene_rect.height < 12 {
        term.draw(|f| paint_footer(f, scene, full_rect, theme, floor_info))?;
        return Ok(());
    }

    let buf_w = scene_rect.width;
    let buf_h = scene_rect.height * 2;
    buf.ensure_size(buf_w, buf_h, theme.surface.bg_fallback);
    let Some(layout) = Layout::compute_with_seed(buf_w, buf_h, scene.max_desks, floor.floor_seed)
    else {
        term.draw(|f| paint_footer(f, scene, full_rect, theme, floor_info))?;
        return Ok(());
    };

    // Bias the router toward the corridor (the office "main aisle") so
    // walkers naturally use the hallway instead of cutting diagonally
    // across the cubicle floor. Cheap call — invalidates the cache only
    // when the zone actually changes (layout resize).
    router.set_preferred_zone(layout.corridor);

    // Pure pixel pass — no ratatui types touched. Pixel pass writes
    // into PoseHistory for every walking/waypoint agent so the next
    // frame's snap-back lookup is fresh.
    render_to_rgb_buffer(
        scene, &layout, pack, now, buf, cache, router, overlay, history, theme, floor,
    );

    // Hit-test the cursor against each agent's current sprite footprint
    // so the tooltip + focus ring know who's under the pointer. Cell-
    // accurate (one terminal cell = 2 vertical pixels in the half-block
    // buffer).
    let hovered = mouse_pos
        .and_then(|(mx, my)| hit_test_agent(scene, &layout, now, router, overlay, history, mx, my));

    // Terminal-flush pass — half-block + widgets, inside ratatui's draw.
    term.draw(|f| {
        paint_footer(f, scene, full_rect, theme, floor_info);
        flush_buffer_to_term(f, buf, scene_rect);
        paint_label_widgets(
            f, scene, &layout, now, router, overlay, history, scene_rect, hovered, theme,
        );
        paint_wall_display(
            f, scene, &layout, scene_rect, now, ticker, theme, floor_info,
        );
        if let Some(door) = layout.door {
            let (current, _) = floor_info.unwrap_or((1, 1));
            paint_elevator_indicator(f, door, current, scene_rect, theme);
        }
        let tooltip_agent = hovered.or(pinned_agent);
        if let (Some(agent_id), Some((mx, my))) = (tooltip_agent, mouse_pos) {
            paint_hover_tooltip(f, scene, agent_id, mx, my, scene_rect, now, theme);
        } else if let Some(agent_id) = pinned_agent {
            paint_hover_tooltip(
                f,
                scene,
                agent_id,
                scene_rect.width / 2,
                scene_rect.height / 2,
                scene_rect,
                now,
                theme,
            );
        }
        if tooltip_agent.is_none() && pinned_agent.is_none() {
            if let Some((mx, my)) = mouse_pos {
                if hit_test_coffee_machine(buf, scene.max_desks, mx, my, floor.floor_seed) {
                    paint_coffee_tooltip(f, mx, my, scene_rect, theme);
                } else if let Some(label) =
                    hit_test_furniture(buf, scene.max_desks, mx, my, floor.floor_seed)
                {
                    paint_furniture_tooltip(f, label, mx, my, scene_rect, theme);
                }
            }
        }
        if let Some(idx) = theme_picker {
            paint_theme_picker(f, idx, full_rect, theme);
        }
    })?;
    Ok(())
}

pub(super) fn paint_theme_picker(
    f: &mut ratatui::Frame<'_>,
    selected: usize,
    bounds: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use crate::tui::theme;
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span as TSpan};
    use ratatui::widgets::{Block, Borders, Clear};

    let w = 30u16;
    let h = (theme::ALL_THEMES.len() as u16 + 2).min(bounds.height);
    let x = bounds.width.saturating_sub(w) / 2;
    let y = bounds.height.saturating_sub(h) / 2;
    let area = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, area);
    let items: Vec<Line> = theme::ALL_THEMES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let prefix = if i == selected { "▸ " } else { "  " };
            let style = if i == selected {
                Style::default()
                    .fg(to_color(theme.ui.neon_brand))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(to_color(theme.ui.label_idle))
            };
            Line::from(TSpan::styled(format!("{prefix}{}", t.name), style))
        })
        .collect();
    let block = Block::default()
        .title(" Theme [↑↓/jk] Enter/Esc ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(to_color(theme.ui.neon_brand)))
        .style(Style::default().bg(to_color(theme.ui.tooltip_bg)));
    f.render_widget(Paragraph::new(items).block(block), area);
}

pub(super) fn paint_footer(
    f: &mut ratatui::Frame<'_>,
    scene: &SceneState,
    full_rect: Rect,
    theme: &crate::tui::theme::Theme,
    floor_info: Option<(usize, usize)>,
) {
    let summary = build_status_summary(scene, full_rect.width, floor_info);
    let footer = Paragraph::new(Span::raw(summary))
        .style(Style::default().fg(to_color(theme.ui.label_idle)));
    f.render_widget(
        footer,
        Rect {
            x: full_rect.x,
            y: full_rect.y + full_rect.height.saturating_sub(1),
            width: full_rect.width,
            height: 1,
        },
    );
}

/// Compose the footer's single-line summary, picking the widest variant
/// (full / medium / minimal) that fits inside `term_width` alongside the
/// fixed-right `[q] quit` suffix. Pure function — drives `paint_footer`
/// and is unit-tested directly.
///
/// Tier breakdown:
///   * **full** (~50+ cells) — total count, per-state counts, top tool
///     names with usage tallies, e.g. `12 agents · 3 active · 2 waiting
///     · 7 idle · Edit×2 Bash×1`.
///   * **medium** (~30+ cells) — compact letters, e.g. `12a · 3A · 2W · 7I`.
///   * **minimal** — just the total, e.g. `12a`.
///   * **fallback** — only the quit hint (any narrower terminal will
///     truncate this naturally).
pub(super) fn build_status_summary(
    scene: &SceneState,
    term_width: u16,
    floor_info: Option<(usize, usize)>,
) -> String {
    let n = scene.agents.len();
    let mut active = 0usize;
    let mut waiting = 0usize;
    let mut idle = 0usize;
    let mut tool_counts: HashMap<&str, usize> = HashMap::new();
    for slot in scene.agents.values() {
        match &slot.state {
            ActivityState::Idle => idle += 1,
            ActivityState::Waiting { .. } => waiting += 1,
            ActivityState::Active { detail, .. } => {
                active += 1;
                if let Some(d) = detail.as_deref() {
                    // Take the leading alphanumeric token as the tool
                    // name — Generic tools format as "Edit src/x.rs",
                    // "Bash: ls"; Task tool shows "Delegating".
                    let token = d.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
                    if !token.is_empty() {
                        *tool_counts.entry(token).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let floor_suffix = match floor_info {
        Some((current, total)) if total > 1 => format!(" F{current}/{total} [\u{2191}\u{2193}]"),
        _ => String::new(),
    };
    let quit_base = " [p]ause [t]heme [+/-]desks [q]uit ";
    let quit = format!("{floor_suffix}{quit_base}");
    let tools_str = {
        // Sort by count desc, then name asc for stable output. Top 4
        // keeps the line bounded — beyond that the listing crowds out
        // the state counts on medium-width terminals.
        let mut tools: Vec<(&&str, &usize)> = tool_counts.iter().collect();
        tools.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        tools
            .iter()
            .take(4)
            .map(|(name, count)| format!("{name}×{count}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let stats_full = if n == 0 {
        " 0 agents ".to_string()
    } else {
        let mut s = format!(" {n} agents · {active} active · {waiting} waiting · {idle} idle");
        if !tools_str.is_empty() {
            s.push_str(" · ");
            s.push_str(&tools_str);
        }
        s.push(' ');
        s
    };
    let stats_medium = format!(" {n}a · {active}A · {waiting}W · {idle}I ");
    let stats_min = format!(" {n}a ");

    let w = term_width as usize;
    let q = quit.len();
    for stats in [&stats_full, &stats_medium, &stats_min] {
        if stats.len() + q <= w {
            let pad = w.saturating_sub(stats.len() + q);
            let mut out = String::with_capacity(w);
            out.push_str(stats);
            for _ in 0..pad {
                out.push(' ');
            }
            out.push_str(&quit);
            return out;
        }
    }
    quit
}

pub(super) fn flush_buffer_to_term_at_offset(
    f: &mut ratatui::Frame<'_>,
    buf: &RgbBuffer,
    scene_rect: Rect,
    y_offset: i32,
) {
    let term_buf = f.buffer_mut();
    let w = buf.width as usize;
    let cell_rows = (buf.height / 2) as usize;
    for cy in 0..cell_rows {
        let target_y = cy as i32 + y_offset;
        if target_y < 0 || target_y >= scene_rect.height as i32 {
            continue;
        }
        for cx in 0..(buf.width as usize) {
            let x = scene_rect.x + cx as u16;
            let y = scene_rect.y + target_y as u16;
            if x >= scene_rect.x + scene_rect.width {
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
}

fn flush_buffer_to_term(f: &mut ratatui::Frame<'_>, buf: &RgbBuffer, scene_rect: Rect) {
    let term_buf = f.buffer_mut();
    let w = buf.width as usize;
    let cell_rows = (buf.height / 2) as usize;
    for cy in 0..cell_rows {
        for cx in 0..(buf.width as usize) {
            let x = scene_rect.x + cx as u16;
            let y = scene_rect.y + cy as u16;
            if x >= scene_rect.x + scene_rect.width || y >= scene_rect.y + scene_rect.height {
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
}

/// Labels above each character — uses `character_anchor` to follow the
/// agent along its current path, color-codes by activity, falls back to
/// disambiguating session-id suffix only when multiple agents share a label.
///
/// `hovered` highlights one agent's label: bright white + bold + leading
/// ▸ marker so the focused character is easy to pick out of a crowd.
#[allow(clippy::too_many_arguments)]
fn paint_label_widgets(
    f: &mut ratatui::Frame<'_>,
    scene: &SceneState,
    layout: &Layout,
    now: SystemTime,
    router: &mut dyn Router,
    overlay: &OccupancyOverlay,
    history: &mut pose::PoseHistory,
    scene_rect: Rect,
    hovered: Option<AgentId>,
    theme: &crate::tui::theme::Theme,
) {
    let agents: Vec<_> = scene.agents.values().cloned().collect();
    let mut label_counts: HashMap<&str, usize> = HashMap::new();
    for agent in &agents {
        *label_counts.entry(&*agent.label).or_insert(0) += 1;
    }
    for agent in &agents {
        let Some(anchor) = character_anchor(agent, layout, now, router, overlay, history) else {
            continue;
        };
        let lx = scene_rect.x + anchor.x.saturating_sub(2);
        let ly = scene_rect.y + (anchor.y / 2).saturating_sub(1);
        let needs_disambig = label_counts.get(&*agent.label).copied().unwrap_or(0) > 1
            && agent.session_id.len() >= 4;
        let raw: std::borrow::Cow<'_, str> = if needs_disambig {
            std::borrow::Cow::Owned(format!("{}·{}", agent.label, &agent.session_id[..4]))
        } else {
            std::borrow::Cow::Borrowed(&*agent.label)
        };
        let display = truncate_label(&raw, (DESK_W + 4) as usize);
        let is_hovered = hovered == Some(agent.agent_id);
        let label_color = if is_hovered {
            Color::White
        } else if agent.exiting_at.is_some() {
            to_color(theme.ui.label_exiting)
        } else {
            match &agent.state {
                ActivityState::Active { .. } => to_color(theme.ui.label_active),
                ActivityState::Waiting { .. } => to_color(theme.ui.label_waiting),
                ActivityState::Idle => to_color(theme.ui.label_idle),
            }
        };
        let text = if is_hovered {
            format!("▸{}", display)
        } else {
            format!("●{}", display)
        };
        let mut style = Style::default().fg(label_color);
        if is_hovered {
            style = style.add_modifier(ratatui::style::Modifier::BOLD);
        }
        let para = Paragraph::new(Span::styled(text, style));
        if let Some(r) = clip_widget_rect(
            Rect {
                x: lx,
                y: ly,
                width: DESK_W + 4,
                height: 1,
            },
            scene_rect,
        ) {
            f.render_widget(para, r);
        }
    }
}

/// Lightweight hit-test for click-to-pin without needing router/overlay state.
/// Uses home desk positions only (no walking agents).
pub fn hit_test_from_tui(
    scene: &SceneState,
    max_desks: usize,
    mx: u16,
    my: u16,
    buf: &RgbBuffer,
) -> Option<AgentId> {
    let buf_h = buf.height;
    let buf_w = buf.width;
    if buf_w < 20 || buf_h < 24 {
        return None;
    }
    let layout = Layout::compute(buf_w, buf_h, max_desks)?;
    const SPRITE_W: u16 = 8;
    const SPRITE_H_CELLS: u16 = 6;
    for agent in scene.agents.values() {
        if agent.desk_index >= layout.home_desks.len() {
            continue;
        }
        let desk = &layout.home_desks[agent.desk_index];
        let ax = desk.x + 1;
        let ay = desk.y.saturating_sub(4);
        let cell_x = ax;
        let cell_y = ay / 2;
        if mx >= cell_x
            && mx < cell_x.saturating_add(SPRITE_W)
            && my >= cell_y
            && my < cell_y.saturating_add(SPRITE_H_CELLS)
        {
            return Some(agent.agent_id);
        }
    }
    None
}

/// Hit-test the mouse cursor against each agent's current sprite footprint.
/// Returns the agent under `(mx, my)` (in terminal cell coordinates), or
/// `None` if no agent occupies that cell.
///
/// The character sprite is 8×12 pixels, which in cell space is 8 cells
/// wide × 6 cells tall (one cell = 2 vertical pixels). We test against
/// that exact bounding box anchored on the agent's `character_anchor`.
#[allow(clippy::too_many_arguments)]
fn hit_test_agent(
    scene: &SceneState,
    layout: &Layout,
    now: SystemTime,
    router: &mut dyn Router,
    overlay: &OccupancyOverlay,
    history: &mut pose::PoseHistory,
    mx: u16,
    my: u16,
) -> Option<AgentId> {
    // Width-in-cells (sprite is 8 px wide; we don't divide x by 2 because
    // each pixel column is one cell column in the half-block grid).
    const SPRITE_W_CELLS: u16 = 8;
    // Height-in-cells: sprite is 12 px tall = 6 cells.
    const SPRITE_H_CELLS: u16 = 6;
    for agent in scene.agents.values() {
        let Some(anchor) = character_anchor(agent, layout, now, router, overlay, history) else {
            continue;
        };
        let cell_x = anchor.x;
        let cell_y = anchor.y / 2;
        if mx >= cell_x
            && mx < cell_x.saturating_add(SPRITE_W_CELLS)
            && my >= cell_y
            && my < cell_y.saturating_add(SPRITE_H_CELLS)
        {
            return Some(agent.agent_id);
        }
    }
    None
}

pub(super) fn paint_elevator_indicator(
    f: &mut ratatui::Frame<'_>,
    door: crate::tui::layout::Point,
    current_floor: usize,
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use ratatui::style::Modifier;
    use ratatui::text::Line;

    let label = format!(" ▲ F{current_floor} ▼ ");
    let label_w = label.len() as u16;
    let door_cell_x = door.x + 8u16.saturating_sub(label_w / 2);
    let door_cell_y = door.y / 2;
    let indicator_y = door_cell_y.saturating_sub(1);

    if let Some(r) = clip_widget_rect(
        Rect {
            x: scene_rect.x + door_cell_x,
            y: scene_rect.y + indicator_y,
            width: label_w,
            height: 1,
        },
        scene_rect,
    ) {
        let style = Style::default()
            .fg(to_color(theme.ui.neon_brand))
            .bg(to_color(theme.ui.tooltip_bg))
            .add_modifier(Modifier::BOLD);
        f.render_widget(Paragraph::new(Line::from(Span::styled(label, style))), r);
    }
}

fn paint_furniture_tooltip(
    f: &mut ratatui::Frame<'_>,
    label: &str,
    mx: u16,
    my: u16,
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use ratatui::text::Line;
    use ratatui::widgets::Block;

    let text = format!(" {} ", label);
    let tip_w = text.len() as u16;
    let tip_h = 1u16;
    let mut tx = mx.saturating_add(2);
    if tx.saturating_add(tip_w) > scene_rect.x + scene_rect.width {
        tx = mx.saturating_sub(tip_w + 1);
    }
    let mut ty = my.saturating_sub(1);
    if ty < scene_rect.y {
        ty = my.saturating_add(1);
    }
    if let Some(r) = clip_widget_rect(
        Rect {
            x: tx,
            y: ty,
            width: tip_w,
            height: tip_h,
        },
        scene_rect,
    ) {
        let block = Block::default().style(Style::default().bg(to_color(theme.ui.tooltip_bg)));
        let line = Line::from(Span::styled(
            text,
            Style::default().fg(to_color(theme.ui.tooltip_title)),
        ));
        f.render_widget(Paragraph::new(line).block(block), r);
    }
}

fn paint_coffee_tooltip(
    f: &mut ratatui::Frame<'_>,
    mx: u16,
    my: u16,
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use ratatui::text::Line;
    use ratatui::widgets::Block;

    let text = " ☕ Buy Ivan a coffee ";
    let tip_w = text.len() as u16;
    let tip_h = 1u16;
    let mut tx = mx.saturating_add(2);
    if tx.saturating_add(tip_w) > scene_rect.x + scene_rect.width {
        tx = mx.saturating_sub(tip_w + 1);
    }
    let mut ty = my.saturating_sub(1);
    if ty < scene_rect.y {
        ty = my.saturating_add(1);
    }
    if let Some(r) = clip_widget_rect(
        Rect {
            x: tx,
            y: ty,
            width: tip_w,
            height: tip_h,
        },
        scene_rect,
    ) {
        let block = Block::default().style(Style::default().bg(to_color(theme.ui.tooltip_bg)));
        let line = Line::from(Span::styled(
            text,
            Style::default().fg(to_color(theme.ui.tooltip_title)),
        ));
        f.render_widget(Paragraph::new(line).block(block), r);
    }
}

/// Hit-test whether the mouse is over the pantry coffee machine.
/// Returns true if `(mx, my)` (terminal cell coords) falls on the coffee
/// machine section of the pantry counter sprite.
pub fn hit_test_coffee_machine(
    buf: &RgbBuffer,
    max_desks: usize,
    mx: u16,
    my: u16,
    floor_seed: u64,
) -> bool {
    let buf_w = buf.width;
    let buf_h = buf.height;
    if buf_w < 20 || buf_h < 24 {
        return false;
    }
    let Some(layout) = Layout::compute_with_seed(buf_w, buf_h, max_desks, floor_seed) else {
        return false;
    };
    let pantry_wp = layout
        .waypoints
        .iter()
        .find(|w| matches!(w.kind, crate::tui::layout::WaypointKind::Pantry));
    let Some(wp) = pantry_wp else {
        return false;
    };
    let (cw, ch) = layout.pantry_counter_size;
    let sprite_x = wp.pos.x.saturating_sub(cw / 2);
    let sprite_y = wp.pos.y.saturating_sub(ch / 2);
    let (coffee_x0, coffee_x1) = if cw >= 32 {
        (sprite_x + 11, sprite_x + 18)
    } else {
        (sprite_x + 8, sprite_x + 13)
    };
    let coffee_y0 = sprite_y;
    let coffee_y1 = sprite_y + ch;
    let cell_y = my * 2;
    mx >= coffee_x0 && mx < coffee_x1 && cell_y >= coffee_y0 && cell_y < coffee_y1
}

/// Hit-test all furniture items in the office. Returns a short label
/// if `(mx, my)` (terminal cell coords) falls on any known item.
/// The coffee machine is handled separately for its click-to-open
/// behavior — this function covers the remaining decorations.
pub fn hit_test_furniture(
    buf: &RgbBuffer,
    max_desks: usize,
    mx: u16,
    my: u16,
    floor_seed: u64,
) -> Option<&'static str> {
    use crate::tui::layout::{PlantKind, PodDecor, WallDecor, WaypointKind, DESK_H, DESK_W};

    let layout = Layout::compute_with_seed(buf.width, buf.height, max_desks, floor_seed)?;
    let px = mx;
    let py = my * 2;

    let hit = |x: u16, y: u16, w: u16, h: u16| -> bool {
        px >= x && px < x.saturating_add(w) && py >= y && py < y.saturating_add(h)
    };

    // Home desks
    for desk in &layout.home_desks {
        if hit(desk.x, desk.y, DESK_W + 2, DESK_H) {
            return Some("Desk");
        }
    }

    // Waypoints
    for wp in &layout.waypoints {
        let (w, h) = match wp.kind {
            WaypointKind::Couch => (16, 7),
            WaypointKind::Pantry => layout.pantry_counter_size,
            WaypointKind::PhoneBooth => (6, 12),
            WaypointKind::StandingDesk => (8, 8),
            WaypointKind::VendingMachine => (4, 6),
            WaypointKind::Printer => (5, 4),
        };
        let wx = wp.pos.x.saturating_sub(w / 2);
        let wy = wp.pos.y.saturating_sub(h / 2);
        if hit(wx, wy, w, h) {
            return Some(match wp.kind {
                WaypointKind::Couch => "Lounge Sofa",
                WaypointKind::Pantry => "Pantry Counter",
                WaypointKind::PhoneBooth => "Phone Booth",
                WaypointKind::StandingDesk => "Standing Desk",
                WaypointKind::VendingMachine => "Vending Machine",
                WaypointKind::Printer => "Printer",
            });
        }
    }

    // Meeting sofas
    for sofa in &layout.meeting_sofas {
        if hit(sofa.x.saturating_sub(8), sofa.y.saturating_sub(3), 16, 7) {
            return Some("Meeting Sofa");
        }
    }

    // Meeting tables
    for t in &layout.meeting_tables {
        if hit(t.x.saturating_sub(6), t.y.saturating_sub(3), 12, 6) {
            return Some("Meeting Table");
        }
    }

    // Pantry table
    if let Some(t) = layout.pantry_table {
        if hit(t.x.saturating_sub(4), t.y.saturating_sub(2), 8, 5) {
            return Some("Pantry Table");
        }
    }

    // Pantry chairs
    for chair in &layout.pantry_chairs {
        if hit(chair.x.saturating_sub(2), chair.y.saturating_sub(2), 3, 3) {
            return Some("Chair");
        }
    }

    // Plants
    for (kind, p) in &layout.plants {
        if hit(p.x.saturating_sub(3), p.y.saturating_sub(3), 6, 6) {
            return Some(match kind {
                PlantKind::Ficus => "Ficus",
                PlantKind::Tall => "Tall Plant",
                PlantKind::Flower => "Flower Pot",
                PlantKind::Succulent => "Succulent",
            });
        }
    }

    // Floor lamp
    if let Some(lamp) = layout.floor_lamp {
        if hit(lamp.x.saturating_sub(2), lamp.y.saturating_sub(3), 4, 6) {
            return Some("Floor Lamp");
        }
    }

    // Wall decor
    for (kind, pos) in &layout.wall_decor {
        let (w, h) = match kind {
            WallDecor::Whiteboard => (14, 11),
            WallDecor::Bookshelf => (10, 8),
            WallDecor::BulletinBoard => (8, 6),
            WallDecor::ExitSign => (6, 3),
            WallDecor::MeetingScreen => (14, 12),
        };
        if hit(pos.x, pos.y, w, h) {
            return Some(match kind {
                WallDecor::Whiteboard => "Whiteboard",
                WallDecor::Bookshelf => "Bookshelf",
                WallDecor::BulletinBoard => "Bulletin Board",
                WallDecor::ExitSign => "Exit Sign",
                WallDecor::MeetingScreen => "Meeting Screen",
            });
        }
    }

    // Pod decor (aisle items)
    for (kind, pos) in &layout.pod_decor {
        let (w, h) = kind.size();
        if hit(
            pos.x.saturating_sub(w / 2),
            pos.y.saturating_sub(h / 2),
            w,
            h,
        ) {
            return Some(match kind {
                PodDecor::PlantTall => "Tall Plant",
                PodDecor::Whiteboard => "Whiteboard",
                PodDecor::Tv => "TV Stand",
                PodDecor::PhoneBooth => "Phone Booth",
                PodDecor::StandingDesk => "Standing Desk",
            });
        }
    }

    // Lounge side table
    if let Some(t) = layout.lounge_side_table {
        if hit(t.x.saturating_sub(3), t.y.saturating_sub(2), 7, 4) {
            return Some("Side Table");
        }
    }

    // Meeting room procedural items (coat rack, doormat)
    if let Some(mr) = layout.meeting_room {
        if mr.width > 20 {
            let cx = mr.x + mr.width - 5;
            let cy = mr.y + mr.height / 2 - 4;
            if hit(cx.saturating_sub(2), cy, 5, 8) {
                return Some("Coat Rack");
            }
        }
        if mr.width > 10 {
            let mat_x = mr.x + mr.width + 1;
            let mat_y = mr.y + mr.height / 2 - 2;
            if hit(mat_x, mat_y, 4, 5) {
                return Some("Doormat");
            }
        }
    }

    // Pantry room procedural items (water cooler, trash bin)
    if let Some(pr) = layout.pantry_room {
        if pr.height > 25 && pr.width > 12 {
            let wx = pr.x + pr.width - 6;
            let wy = pr.y + 8;
            if hit(wx, wy, 3, 6) {
                return Some("Water Cooler");
            }
        }
        if pr.height > 20 {
            let tx = pr.x + 3;
            let ty = pr.y + pr.height - 14;
            if hit(tx, ty, 4, 5) {
                return Some("Trash Bin");
            }
        }
    }

    // Door / elevator (16×14 sprite)
    if let Some(d) = layout.door {
        if hit(d.x, d.y, 16, 14) {
            return Some("Elevator");
        }
    }

    None
}

/// Floating detail panel painted near the cursor when an agent is hovered.
/// Shows the label, source, state, current tool detail, cwd, and session
/// id. Positioned to avoid the cursor itself and the screen edges.
#[allow(clippy::too_many_arguments)]
fn paint_hover_tooltip(
    f: &mut ratatui::Frame<'_>,
    scene: &SceneState,
    agent_id: AgentId,
    mx: u16,
    my: u16,
    scene_rect: Rect,
    now: SystemTime,
    theme: &crate::tui::theme::Theme,
) {
    let Some(agent) = scene.agents.get(&agent_id) else {
        return;
    };

    // Build the tooltip lines.
    let (state_label, state_detail, state_color) = match &agent.state {
        ActivityState::Idle => ("Idle", String::new(), to_color(theme.ui.label_idle)),
        ActivityState::Active { detail, .. } => (
            "Active",
            detail.as_deref().unwrap_or("").to_string(),
            to_color(theme.ui.label_active),
        ),
        ActivityState::Waiting { reason } => (
            "Waiting",
            reason.to_string(),
            to_color(theme.ui.label_waiting),
        ),
    };
    let cwd_short = agent
        .cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("(unknown)");

    let session_secs = now
        .duration_since(agent.created_at)
        .unwrap_or_default()
        .as_secs();
    let duration_str = if session_secs >= 3600 {
        format!("{}h{}m", session_secs / 3600, (session_secs % 3600) / 60)
    } else if session_secs >= 60 {
        format!("{}m", session_secs / 60)
    } else {
        "<1m".to_string()
    };
    let active_str = if session_secs >= 5 {
        let pct = (agent.active_ms / 1000)
            .checked_mul(100)
            .and_then(|n| n.checked_div(session_secs))
            .map(|p| p.min(100))
            .unwrap_or(0);
        format!("{pct}%")
    } else {
        "--%".to_string()
    };

    let mut lines: Vec<ratatui::text::Line> = Vec::new();
    lines.push(ratatui::text::Line::from(Span::styled(
        format!(" {} ", agent.label),
        Style::default()
            .fg(to_color(theme.ui.tooltip_title))
            .add_modifier(ratatui::style::Modifier::BOLD),
    )));
    lines.push(ratatui::text::Line::from(vec![
        Span::raw(" ●  "),
        Span::styled(state_label, Style::default().fg(state_color)),
    ]));
    if !state_detail.is_empty() {
        let trimmed: String = state_detail.chars().take(34).collect();
        lines.push(ratatui::text::Line::from(Span::styled(
            format!("    {}", trimmed),
            Style::default().fg(to_color(theme.ui.tooltip_text)),
        )));
    }
    lines.push(ratatui::text::Line::from(Span::styled(
        format!(" 📁 {}", cwd_short),
        Style::default().fg(to_color(theme.ui.tooltip_text)),
    )));
    lines.push(ratatui::text::Line::from(Span::styled(
        format!(
            " ⏱ {} · {} calls · {}% active",
            duration_str, agent.tool_call_count, active_str
        ),
        Style::default().fg(to_color(theme.ui.tooltip_dim)),
    )));

    let lines_h = lines.len() as u16;
    let max_w = lines.iter().map(|l| l.width() as u16).max().unwrap_or(20) + 2;
    let tip_w = max_w.min(scene_rect.width).max(18);
    let tip_h = lines_h;

    // Place the tooltip to the RIGHT and BELOW the cursor when there's
    // room; otherwise flip to the other side so it stays on-screen.
    let mut tx = mx.saturating_add(2);
    if tx.saturating_add(tip_w) > scene_rect.x + scene_rect.width {
        tx = mx.saturating_sub(tip_w + 1);
    }
    let mut ty = my.saturating_add(1);
    if ty.saturating_add(tip_h) > scene_rect.y + scene_rect.height {
        ty = my.saturating_sub(tip_h).max(scene_rect.y);
    }
    let rect = Rect {
        x: tx,
        y: ty,
        width: tip_w,
        height: tip_h,
    };
    let Some(clipped) = clip_widget_rect(rect, scene_rect) else {
        return;
    };

    let para = Paragraph::new(lines).style(
        Style::default()
            .bg(to_color(theme.ui.tooltip_bg))
            .fg(Color::White),
    );
    f.render_widget(ratatui::widgets::Clear, clipped);
    f.render_widget(para, clipped);
}

/// Wall-mounted status display rendered in the wall band, right of center.
/// Shows branding + version on top line, agent state dots + uptime on
/// bottom line. The GitHub star link uses OSC 8 hyperlinks — clicking it
/// in supported terminals (iTerm2, Ghostty, Kitty, WezTerm) opens the
/// browser.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_wall_display(
    f: &mut ratatui::Frame<'_>,
    scene: &SceneState,
    _layout: &Layout,
    scene_rect: Rect,
    now: SystemTime,
    ticker: &TickerQueue,
    theme: &crate::tui::theme::Theme,
    floor_info: Option<(usize, usize)>,
) {
    use ratatui::style::Modifier;
    use ratatui::text::Line;

    let cell_x = scene_rect.x + 2;
    let cell_y = scene_rect.y + 1;

    let live: Vec<&AgentSlot> = scene
        .agents
        .values()
        .filter(|a| a.exiting_at.is_none())
        .collect();
    let active = live
        .iter()
        .filter(|a| matches!(a.state, ActivityState::Active { .. }))
        .count();
    let waiting = live
        .iter()
        .filter(|a| matches!(a.state, ActivityState::Waiting { .. }))
        .count();
    let idle = live.len() - active - waiting;

    let version = env!("CARGO_PKG_VERSION");
    let mut top_spans = vec![
        Span::styled(
            format!("ascii-agents v{version}"),
            Style::default()
                .fg(to_color(theme.ui.neon_brand))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            "★ Star",
            Style::default()
                .fg(to_color(theme.ui.neon_star))
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some((current, total)) = floor_info {
        if total > 1 {
            top_spans.push(Span::raw("  "));
            top_spans.push(Span::styled(
                format!("Floor {current}/{total}"),
                Style::default().fg(to_color(theme.ui.neon_brand)),
            ));
        }
    }
    let top_line = Line::from(top_spans);

    let oldest = live
        .iter()
        .filter_map(|a| now.duration_since(a.created_at).ok())
        .max()
        .unwrap_or_default();
    let uptime_secs = oldest.as_secs();
    let uptime_str = if uptime_secs >= 3600 {
        format!("↑{}h{}m", uptime_secs / 3600, (uptime_secs % 3600) / 60)
    } else if uptime_secs >= 60 {
        format!("↑{}m", uptime_secs / 60)
    } else {
        "↑<1m".to_string()
    };

    let bot_line = Line::from(vec![
        Span::styled(
            "●".repeat(active),
            Style::default().fg(to_color(theme.ui.label_active)),
        ),
        Span::styled(
            "●".repeat(waiting),
            Style::default().fg(to_color(theme.ui.label_waiting)),
        ),
        Span::styled(
            "●".repeat(idle),
            Style::default().fg(to_color(theme.ui.label_idle)),
        ),
        Span::raw("  "),
        Span::styled(uptime_str, Style::default().fg(Color::DarkGray)),
    ]);

    let ticker_width = 28usize;
    let visible = ticker.visible(ticker_width, now);
    let ticker_line = Line::from(Span::styled(
        visible,
        Style::default().fg(to_color(theme.ui.neon_ticker)),
    ));

    let w = 30u16;
    if let Some(r) = clip_widget_rect(
        Rect {
            x: cell_x,
            y: cell_y,
            width: w,
            height: 3,
        },
        scene_rect,
    ) {
        f.render_widget(Paragraph::new(vec![top_line, bot_line, ticker_line]), r);
    }
}

/// Fit a label into `budget` chars without losing the `·xxxx` session-id
/// disambiguation suffix that the reducer appends to colliding cwds.
/// Truncates from the base (left side of the `·`), not from the suffix —
/// otherwise the disambig becomes useless ("TikTok-Android·a" tells us
/// nothing the base alone wouldn't).
fn truncate_label(label: &str, budget: usize) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    if label.chars().count() <= budget {
        return Cow::Borrowed(label);
    }
    if let Some(sep_byte) = label.rfind('·') {
        let suffix = &label[sep_byte..];
        let suffix_len = suffix.chars().count();
        if suffix_len < budget {
            let base = &label[..sep_byte];
            let base_take = budget - suffix_len;
            let truncated: String = base.chars().take(base_take).collect();
            return Cow::Owned(format!("{truncated}{suffix}"));
        }
    }
    Cow::Owned(label.chars().take(budget).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_widget_rect_fully_inside() {
        let r = Rect {
            x: 2,
            y: 2,
            width: 4,
            height: 4,
        };
        let b = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        assert_eq!(clip_widget_rect(r, b), Some(r));
    }

    #[test]
    fn clip_widget_rect_fully_outside_right() {
        let r = Rect {
            x: 80,
            y: 0,
            width: 10,
            height: 5,
        };
        let b = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        assert_eq!(clip_widget_rect(r, b), None);
    }

    #[test]
    fn clip_widget_rect_partially_overflows_right() {
        let r = Rect {
            x: 75,
            y: 0,
            width: 10,
            height: 5,
        };
        let b = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let clipped = clip_widget_rect(r, b).unwrap();
        assert_eq!(clipped.x, 75);
        assert_eq!(clipped.width, 5);
    }

    #[test]
    fn clip_widget_rect_zero_size_returns_none() {
        let r = Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 5,
        };
        let b = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        assert_eq!(clip_widget_rect(r, b), None);
    }

    #[test]
    fn truncate_label_passes_short_labels_through() {
        assert_eq!(truncate_label("hello", 16), "hello");
    }

    #[test]
    fn truncate_label_preserves_disambig_suffix() {
        // 19 chars > 16 budget → must drop chars from the base, NOT the suffix.
        let out = truncate_label("TikTok-Android·a09a", 16);
        assert_eq!(out.chars().count(), 16);
        assert!(out.ends_with("·a09a"), "suffix lost: {out}");
        assert!(out.starts_with("TikTok"), "base over-truncated: {out}");
    }

    #[test]
    fn truncate_label_falls_back_to_plain_truncate_when_no_separator() {
        let out = truncate_label("a-very-long-project-name", 8);
        assert_eq!(out, "a-very-l");
    }

    // --- build_status_summary ---------------------------------------------

    use ascii_agents_core::source::Activity;
    use ascii_agents_core::{AgentId, AgentSlot};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn slot_with(state: ActivityState, label: &str) -> AgentSlot {
        AgentSlot {
            agent_id: AgentId::from_transcript_path(&format!("/p/{label}.jsonl")),
            source: Arc::from("claude-code"),
            session_id: Arc::from("s"),
            cwd: Arc::from(PathBuf::from("/p").as_path()),
            label: Arc::from(label),
            state,
            state_started_at: SystemTime::UNIX_EPOCH,
            created_at: SystemTime::UNIX_EPOCH,
            last_event_at: SystemTime::UNIX_EPOCH,
            exiting_at: None,
            pending_idle_at: None,
            desk_index: 0,
            tool_call_count: 0,
            active_ms: 0,
            unknown_cwd: false,
            parent_id: None,
        }
    }
    fn active_with(detail: &str, label: &str) -> AgentSlot {
        slot_with(
            ActivityState::Active {
                activity: Activity::Typing,
                tool_use_id: Some(Arc::from("t")),
                detail: Some(Arc::from(detail)),
            },
            label,
        )
    }
    fn waiting(label: &str) -> AgentSlot {
        slot_with(
            ActivityState::Waiting {
                reason: Arc::from("perm"),
            },
            label,
        )
    }
    fn idle(label: &str) -> AgentSlot {
        slot_with(ActivityState::Idle, label)
    }
    fn scene_of(slots: Vec<AgentSlot>) -> SceneState {
        let mut s = SceneState::new(16);
        for slot in slots {
            s.agents.insert(slot.agent_id, slot);
        }
        s
    }

    const QUIT_SUFFIX: &str = " [p]ause [t]heme [+/-]desks [q]uit ";

    #[test]
    fn footer_zero_agents_shows_zero_count_and_quit() {
        let s = scene_of(vec![]);
        let line = build_status_summary(&s, 80, None);
        assert!(line.contains("0 agents"), "missing zero count: {line:?}");
        assert!(line.ends_with(QUIT_SUFFIX), "missing quit suffix: {line:?}");
        assert_eq!(line.len(), 80, "should pad to full width: {line:?}");
    }

    #[test]
    fn footer_full_width_shows_state_breakdown_and_tools() {
        let s = scene_of(vec![
            active_with("Edit src/a.rs", "a"),
            active_with("Edit src/b.rs", "b"),
            active_with("Bash: ls", "c"),
            waiting("d"),
            waiting("e"),
            idle("f"),
            idle("g"),
            idle("h"),
        ]);
        let line = build_status_summary(&s, 120, None);
        // Per-state counts present.
        assert!(line.contains("8 agents"), "{line:?}");
        assert!(line.contains("3 active"), "{line:?}");
        assert!(line.contains("2 waiting"), "{line:?}");
        assert!(line.contains("3 idle"), "{line:?}");
        // Top tools by count, ordered desc: Edit×2 should come before Bash×1.
        let edit_pos = line.find("Edit×2").expect("Edit×2 present");
        let bash_pos = line.find("Bash×1").expect("Bash×1 present");
        assert!(
            edit_pos < bash_pos,
            "tools should sort by count desc: {line:?}"
        );
    }

    #[test]
    fn footer_medium_width_drops_to_compact_letters() {
        let s = scene_of(vec![
            active_with("Edit src/a.rs", "a"),
            waiting("b"),
            idle("c"),
        ]);
        let line = build_status_summary(&s, 60, None);
        assert!(
            line.contains("3a") && line.contains("1A"),
            "expected medium tier letters: {line:?}"
        );
        assert!(
            !line.contains("3 agents · "),
            "full tier should not fit at width 60: {line:?}"
        );
        assert!(line.ends_with(QUIT_SUFFIX), "{line:?}");
    }

    #[test]
    fn footer_minimal_width_keeps_total_and_quit_only() {
        let s = scene_of(vec![idle("a"), idle("b")]);
        let w = QUIT_SUFFIX.len() + 6;
        let line = build_status_summary(&s, w as u16, None);
        assert!(line.contains("2a"), "expected minimal tier: {line:?}");
        assert!(line.ends_with(QUIT_SUFFIX), "{line:?}");
        assert_eq!(line.len(), w);
    }

    #[test]
    fn footer_collapses_to_quit_only_below_minimal_threshold() {
        let s = scene_of(vec![idle("a")]);
        let w = QUIT_SUFFIX.len();
        let line = build_status_summary(&s, w as u16, None);
        assert_eq!(line, QUIT_SUFFIX);
    }

    #[test]
    fn footer_caps_tool_breakdown_at_four_entries() {
        let s = scene_of(vec![
            active_with("Edit x", "a"),
            active_with("Bash x", "b"),
            active_with("Read x", "c"),
            active_with("Write x", "d"),
            active_with("Grep x", "e"),
            active_with("Glob x", "f"),
        ]);
        let line = build_status_summary(&s, 200, None);
        // Six distinct tools, but only 4 should appear. Count `×` markers.
        let crosses = line.matches('×').count();
        assert_eq!(crosses, 4, "expected ≤4 tools in breakdown: {line:?}");
    }

    #[test]
    fn coffee_machine_hit_test_returns_false_for_tiny_buffer() {
        let buf = RgbBuffer::filled(10, 10, Rgb(0, 0, 0));
        assert!(!hit_test_coffee_machine(&buf, 4, 5, 5, 0));
    }

    #[test]
    fn coffee_machine_hit_test_returns_false_for_origin() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        assert!(!hit_test_coffee_machine(&buf, 4, 0, 0, 0));
    }

    #[test]
    fn coffee_machine_hit_test_returns_true_for_machine_area() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        let layout = Layout::compute(160, 200, 4).expect("layout");
        let pantry_wp = layout
            .waypoints
            .iter()
            .find(|w| w.kind == crate::tui::layout::WaypointKind::Pantry)
            .expect("pantry");
        let (cw, ch) = layout.pantry_counter_size;
        let sprite_x = pantry_wp.pos.x.saturating_sub(cw / 2);
        let sprite_y = pantry_wp.pos.y.saturating_sub(ch / 2);
        let mid_x = if cw >= 32 {
            sprite_x + 14
        } else {
            sprite_x + 10
        };
        let mid_cell_y = (sprite_y + ch / 2) / 2;
        assert!(
            hit_test_coffee_machine(&buf, 4, mid_x, mid_cell_y, 0),
            "expected hit at coffee machine area ({mid_x}, {mid_cell_y})"
        );
    }

    #[test]
    fn furniture_hit_test_returns_none_for_empty_space() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        assert_eq!(hit_test_furniture(&buf, 4, 80, 50, 0), None);
    }

    #[test]
    fn furniture_hit_test_finds_desk() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        let layout = Layout::compute(160, 200, 4).expect("layout");
        let desk = layout.home_desks.first().expect("desk");
        let cell_y = (desk.y + 2) / 2;
        assert_eq!(
            hit_test_furniture(&buf, 4, desk.x + 2, cell_y, 0),
            Some("Desk")
        );
    }

    #[test]
    fn furniture_hit_test_finds_elevator() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        let layout = Layout::compute(160, 200, 4).expect("layout");
        let door = layout.door.expect("door");
        let cell_y = (door.y + 7) / 2;
        assert_eq!(
            hit_test_furniture(&buf, 4, door.x + 8, cell_y, 0),
            Some("Elevator")
        );
    }

    #[test]
    fn furniture_hit_test_finds_meeting_table() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        let layout = Layout::compute(160, 200, 4).expect("layout");
        let table = layout.meeting_tables.first().expect("table");
        let cell_y = table.y / 2;
        assert_eq!(
            hit_test_furniture(&buf, 4, table.x, cell_y, 0),
            Some("Meeting Table")
        );
    }

    #[test]
    fn furniture_hit_test_respects_floor_seed() {
        let buf = RgbBuffer::filled(160, 200, Rgb(0, 0, 0));
        // Seed 1 = open plan (no meeting room)
        let layout = Layout::compute_with_seed(160, 200, 4, 1).expect("layout");
        assert!(layout.meeting_tables.is_empty());
        // No meeting table at seed=1, so hitting the same coords should not
        // return "Meeting Table"
        let layout0 = Layout::compute(160, 200, 4).expect("layout");
        if let Some(table) = layout0.meeting_tables.first() {
            let cell_y = table.y / 2;
            assert_ne!(
                hit_test_furniture(&buf, 4, table.x, cell_y, 1),
                Some("Meeting Table"),
            );
        }
    }
}
