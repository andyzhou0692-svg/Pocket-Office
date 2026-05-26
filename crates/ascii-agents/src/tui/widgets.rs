//! Ratatui widget paint functions: footer, labels, wall display, tooltips,
//! ticker queue, and theme picker overlay.

use std::collections::HashMap;
use std::time::SystemTime;

use ascii_agents_core::sprite::Rgb;
use ascii_agents_core::state::ActivityState;
use ascii_agents_core::walkable::OccupancyOverlay;
use ascii_agents_core::{AgentId, AgentSlot, SceneState};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::tui::layout::{Layout, DESK_W};
use crate::tui::pathfind::Router;
use crate::tui::pixel_painter::character_anchor;
use crate::tui::pose;
use crate::tui::renderer::clip_widget_rect;

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
        let chars: Vec<char> = self.buffer.chars().collect();
        let len = chars.len();
        let offset = (elapsed_ms / 150) as usize % len;
        (0..width).map(|i| chars[(offset + i) % len]).collect()
    }
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
    let quit_base = " [p]ause [t]heme [q]uit ";
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

/// Labels above each character — uses `character_anchor` to follow the
/// agent along its current path, color-codes by activity, falls back to
/// disambiguating session-id suffix only when multiple agents share a label.
///
/// `hovered` highlights one agent's label: bright white + bold + leading
/// ▸ marker so the focused character is easy to pick out of a crowd.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_label_widgets(
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

/// Floating detail panel painted near the cursor when an agent is hovered.
/// Shows the label, source, state, current tool detail, cwd, and session
/// id. Positioned to avoid the cursor itself and the screen edges.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_hover_tooltip(
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
        format!(" \u{1f4c1} {}", cwd_short),
        Style::default().fg(to_color(theme.ui.tooltip_text)),
    )));
    lines.push(ratatui::text::Line::from(Span::styled(
        format!(
            " \u{23f1} {} \u{00b7} {} calls \u{00b7} {} active",
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
            "\u{2605} Star",
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
        format!(
            "\u{2191}{}h{}m",
            uptime_secs / 3600,
            (uptime_secs % 3600) / 60
        )
    } else if uptime_secs >= 60 {
        format!("\u{2191}{}m", uptime_secs / 60)
    } else {
        "\u{2191}<1m".to_string()
    };

    let bot_line = Line::from(vec![
        Span::styled(
            "\u{25cf}".repeat(active),
            Style::default().fg(to_color(theme.ui.label_active)),
        ),
        Span::styled(
            "\u{25cf}".repeat(waiting),
            Style::default().fg(to_color(theme.ui.label_waiting)),
        ),
        Span::styled(
            "\u{25cf}".repeat(idle),
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

pub(crate) fn paint_coffee_tooltip(
    f: &mut ratatui::Frame<'_>,
    mx: u16,
    my: u16,
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use ratatui::text::Line;
    use ratatui::widgets::Block;

    let text = " \u{2615} Buy Ivan a coffee ";
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

pub(crate) fn paint_furniture_tooltip(
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

/// Cat tooltip — state-dependent text rendered near the cursor.
/// Same visual style as furniture tooltips (dark bg, light text).
pub(crate) fn paint_cat_tooltip(
    f: &mut ratatui::Frame<'_>,
    anim_name: &str,
    is_on_cooldown: bool,
    mx: u16,
    my: u16,
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use ratatui::text::Line;
    use ratatui::widgets::Block;

    let text = if is_on_cooldown {
        " purr... "
    } else {
        match anim_name {
            "cat_sleep" => " Shhh... sleeping ",
            "cat_sit" => " Pet me! ",
            "cat_walk" => " Office Cat (walking) ",
            _ => " Office Cat ",
        }
    };
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
    if let Some(sep_byte) = label.rfind('\u{00b7}') {
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

pub(super) fn paint_elevator_indicator(
    f: &mut ratatui::Frame<'_>,
    door: crate::tui::layout::Point,
    current_floor: usize,
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    use ratatui::style::Modifier;
    use ratatui::text::Line;

    let label = format!(" \u{25b2} F{current_floor} \u{25bc} ");
    let label_w = label.len() as u16;
    let door_cell_x = door.x + 8u16.saturating_sub(label_w / 2);
    let door_cell_y = door.y / 2;
    let indicator_y = door_cell_y.saturating_sub(1);

    if let Some(r) = crate::tui::renderer::clip_widget_rect(
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

/// Paint chitchat speech bubbles above agents who are chatting at a
/// social waypoint. Each bubble is a small Paragraph with the speaker's
/// line of text, positioned above the agent's sprite head.
pub fn paint_chitchat_bubbles(
    f: &mut ratatui::Frame<'_>,
    bubbles: &[crate::tui::chitchat::ChitchatBubble],
    scene_rect: Rect,
    theme: &crate::tui::theme::Theme,
) {
    for bubble in bubbles {
        let text = format!(" {} ", bubble.text);
        let tip_w = text.len() as u16;
        let tip_h = 1u16;

        // Convert pixel anchor to cell coords.
        let cell_x = scene_rect.x + bubble.anchor.x;
        let cell_y = scene_rect.y + bubble.anchor.y / 2;

        // Position above the sprite head.
        let bx = cell_x.saturating_sub(tip_w / 2);
        let by = cell_y.saturating_sub(3);

        if let Some(r) = clip_widget_rect(
            Rect {
                x: bx,
                y: by,
                width: tip_w,
                height: tip_h,
            },
            scene_rect,
        ) {
            let style = Style::default()
                .bg(to_color(theme.ui.tooltip_bg))
                .fg(Color::White);
            f.render_widget(Paragraph::new(Span::styled(text, style)), r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ascii_agents_core::source::Activity;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn truncate_label_passes_short_labels_through() {
        assert_eq!(truncate_label("hello", 16), "hello");
    }

    #[test]
    fn truncate_label_preserves_disambig_suffix() {
        // 19 chars > 16 budget -- must drop chars from the base, NOT the suffix.
        let out = truncate_label("TikTok-Android\u{00b7}a09a", 16);
        assert_eq!(out.chars().count(), 16);
        assert!(out.ends_with("\u{00b7}a09a"), "suffix lost: {out}");
        assert!(out.starts_with("TikTok"), "base over-truncated: {out}");
    }

    #[test]
    fn truncate_label_falls_back_to_plain_truncate_when_no_separator() {
        let out = truncate_label("a-very-long-project-name", 8);
        assert_eq!(out, "a-very-l");
    }

    // --- build_status_summary ---------------------------------------------

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
            floor_idx: 0,
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
        let mut s = SceneState::uniform(16);
        for slot in slots {
            s.agents.insert(slot.agent_id, slot);
        }
        s
    }

    const QUIT_SUFFIX: &str = " [p]ause [t]heme [q]uit ";

    #[test]
    fn footer_zero_agents() {
        let s = scene_of(vec![]);
        let line = build_status_summary(&s, 80, None);
        assert_eq!(line.len(), 80, "should pad to full width");
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_single_idle_agent() {
        let s = scene_of(vec![idle("myproject")]);
        let line = build_status_summary(&s, 80, None);
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_full_width_mixed_states() {
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
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_medium_width_compact() {
        let s = scene_of(vec![
            active_with("Edit src/a.rs", "a"),
            waiting("b"),
            idle("c"),
        ]);
        let line = build_status_summary(&s, 60, None);
        assert!(
            !line.contains("3 agents"),
            "full tier should not fit at width 60"
        );
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_minimal_width() {
        let s = scene_of(vec![idle("a"), idle("b")]);
        let w = QUIT_SUFFIX.len() + 6;
        let line = build_status_summary(&s, w as u16, None);
        assert_eq!(line.len(), w);
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_quit_only_below_threshold() {
        let s = scene_of(vec![idle("a")]);
        let w = QUIT_SUFFIX.len();
        let line = build_status_summary(&s, w as u16, None);
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_caps_tools_at_four() {
        let s = scene_of(vec![
            active_with("Edit x", "a"),
            active_with("Bash x", "b"),
            active_with("Read x", "c"),
            active_with("Write x", "d"),
            active_with("Grep x", "e"),
            active_with("Glob x", "f"),
        ]);
        let line = build_status_summary(&s, 200, None);
        // Six distinct tools, but only 4 should appear.
        let crosses = line.matches('\u{00d7}').count();
        assert_eq!(crosses, 4, "expected <=4 tools in breakdown");
        insta::assert_snapshot!(line);
    }

    #[test]
    fn footer_with_floor_info() {
        let s = scene_of(vec![idle("a"), idle("b")]);
        let line = build_status_summary(&s, 120, Some((2, 3)));
        insta::assert_snapshot!(line);
    }
}
