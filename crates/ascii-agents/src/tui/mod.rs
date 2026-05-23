pub mod embedded_pack;
pub mod frame_cache;
pub mod layout;
pub mod pathfind;
pub mod pixel_painter;
pub mod pose;
pub mod renderer;
pub mod tui_renderer;

use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use ascii_agents_core::Renderer;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};

use renderer::{setup_terminal, teardown_terminal};
use tui_renderer::TuiRenderer;

use crate::runtime::SceneRx;

pub async fn run_tui(mut scene_rx: SceneRx, pack_dir: Option<std::path::PathBuf>) -> Result<()> {
    let pack = embedded_pack::load_sprite_pack(pack_dir)?;
    let term = setup_terminal()?;
    let mut renderer = TuiRenderer::new(term);
    // Track the static-mask signature so the router drops its cache when the
    // obstacle set changes (terminal resize, agent count crosses the
    // visible-desk threshold). Dynamic occupancy churn is handled inside
    // the router via overlay signature.
    let mut last_layout_sig: Option<(u16, u16, usize)> = None;

    let tick = Duration::from_millis(33); // ~30 fps
    let result: Result<()> = (async {
        loop {
            let now = SystemTime::now();
            // O(1) pointer read — no lock, no clone of SceneState's contents.
            let snapshot = scene_rx.borrow_and_update().clone();
            renderer.evict_missing(&snapshot);
            let sig = (
                renderer.buf().width,
                renderer.buf().height,
                snapshot.max_desks,
            );
            if last_layout_sig != Some(sig) {
                renderer.invalidate_routes();
                last_layout_sig = Some(sig);
            }
            // TuiRenderer::render rebuilds the overlay internally from
            // current agent positions before computing routed poses, so the
            // router routes around live agents and characters don't overlap.
            renderer.render(&snapshot, &pack, now)?;

            let start = Instant::now();
            // Drain every event that arrived during this tick. Mouse moves
            // can fire 50-200/s on a fast cursor — we want the latest
            // position before the next frame, not just the first one.
            let mut polled = event::poll(tick)?;
            let mut quit = false;
            while polled {
                match event::read()? {
                    Event::Key(k) => match (k.code, k.modifiers) {
                        (KeyCode::Char('q'), _)
                        | (KeyCode::Esc, _)
                        | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            quit = true;
                        }
                        _ => {}
                    },
                    Event::Mouse(m) => {
                        // Track move + drag positions; ignore scroll/button
                        // press events for now (no other interaction yet).
                        if matches!(
                            m.kind,
                            MouseEventKind::Moved
                                | MouseEventKind::Drag(_)
                                | MouseEventKind::Down(_)
                        ) {
                            renderer.set_mouse_pos(Some((m.column, m.row)));
                        }
                    }
                    _ => {}
                }
                polled = event::poll(Duration::from_millis(0))?;
            }
            if quit {
                break;
            }
            let elapsed = start.elapsed();
            if let Some(rem) = tick.checked_sub(elapsed) {
                tokio::time::sleep(rem).await;
            }
            tokio::task::yield_now().await;
        }
        Ok(())
    })
    .await;

    teardown_terminal(&mut renderer.terminal)?;
    result
}
