//! Golden image regression tests.
//!
//! Render deterministic scenes via `draw_scene`, convert the `RgbBuffer` to a
//! PNG in memory using the `image` crate, and snapshot with
//! `insta::assert_binary_snapshot!`. Insta stores the reference PNGs and
//! handles diffing on update.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use ascii_agents::tui::embedded_pack::load_sprite_pack;
use ascii_agents::tui::floor::FloorMeta;
use ascii_agents::tui::frame_cache::FrameCache;
use ascii_agents::tui::pathfind::AStarRouter;
use ascii_agents::tui::pose::PoseHistory;
use ascii_agents::tui::renderer::{draw_scene, DrawCtx, TickerQueue};
use ascii_agents::tui::theme;
use ascii_agents_core::source::Activity;
use ascii_agents_core::sprite::{Rgb, RgbBuffer};
use ascii_agents_core::state::ActivityState;
use ascii_agents_core::walkable::OccupancyOverlay;
use ascii_agents_core::{AgentId, AgentSlot, SceneState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Deterministic timestamp shared by all tests.
const NOW_SECS: u64 = 1_716_286_800;

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(NOW_SECS)
}

fn empty_scene() -> SceneState {
    SceneState::new(12)
}

fn populated_scene(now: SystemTime) -> SceneState {
    let mut s = SceneState::new(12);
    let age_offset = Duration::from_secs(60);
    let cases: &[(&str, ActivityState)] = &[
        (
            "agent-a",
            ActivityState::Active {
                activity: Activity::Typing,
                tool_use_id: Some("tu_a".into()),
                detail: Some("Write".into()),
            },
        ),
        ("agent-b", ActivityState::Idle),
        (
            "agent-c",
            ActivityState::Waiting {
                reason: "perm?".into(),
            },
        ),
        ("agent-d", ActivityState::Idle),
    ];
    for (i, (key, state)) in cases.iter().enumerate() {
        let id = AgentId::from_transcript_path(&format!("/demo/{key}.jsonl"));
        let created_at = now - age_offset;
        s.agents.insert(
            id,
            AgentSlot {
                agent_id: id,
                source: Arc::from("claude-code"),
                session_id: Arc::from(format!("session-{i}").as_str()),
                cwd: Arc::from(PathBuf::from("/demo").as_path()),
                label: Arc::from(*key),
                state: state.clone(),
                state_started_at: now,
                last_event_at: now,
                created_at,
                exiting_at: None,
                pending_idle_at: None,
                desk_index: i,
                tool_call_count: 0,
                active_ms: 0,
                unknown_cwd: false,
                parent_id: None,
            },
        );
    }
    s
}

/// Render a scene and return the pixel buffer as a PNG byte vector.
fn render_to_png(
    scene: &SceneState,
    now: SystemTime,
    t: &theme::Theme,
    floor_seed: u64,
) -> Vec<u8> {
    let backend = TestBackend::new(96, 36);
    let mut term = Terminal::new(backend).unwrap();
    let mut buf = RgbBuffer::filled(0, 0, Rgb(0, 0, 0));
    let pack = load_sprite_pack(None).unwrap();
    let mut cache = FrameCache::new();
    let mut router = AStarRouter::new();
    let mut overlay = OccupancyOverlay::new();
    let ticker = TickerQueue::new();
    let mut history = PoseHistory::new();
    let floor = if floor_seed == 0 {
        FloorMeta::ground()
    } else {
        FloorMeta::for_floor(floor_seed as usize, 4)
    };
    let mut draw_ctx = DrawCtx {
        buf: &mut buf,
        cache: &mut cache,
        router: &mut router,
        overlay: &mut overlay,
        history: &mut history,
        mouse_pos: None,
        pinned_agent: None,
        ticker: &ticker,
        theme: t,
        theme_picker: None,
        floor_info: None,
        floor,
    };
    draw_scene(&mut term, scene, &pack, now, &mut draw_ctx).unwrap();

    let w = draw_ctx.buf.width as u32;
    let h = draw_ctx.buf.height as u32;
    let mut img = image::RgbImage::new(w, h);
    for (i, px) in draw_ctx.buf.pixels.iter().enumerate() {
        let x = (i as u32) % w;
        let y = (i as u32) / w;
        img.put_pixel(x, y, image::Rgb([px.0, px.1, px.2]));
    }
    let mut png_bytes: Vec<u8> = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    png_bytes
}

#[test]
fn golden_empty_office() {
    let scene = empty_scene();
    let png = render_to_png(&scene, now(), &theme::NORMAL, 0);
    insta::assert_binary_snapshot!("golden_empty_office.png", png);
}

#[test]
fn golden_populated_office() {
    let n = now();
    let scene = populated_scene(n);
    let png = render_to_png(&scene, n, &theme::NORMAL, 0);
    insta::assert_binary_snapshot!("golden_populated_office.png", png);
}

#[test]
fn golden_cyberpunk_theme() {
    let n = now();
    let scene = populated_scene(n);
    let png = render_to_png(&scene, n, &theme::CYBERPUNK, 0);
    insta::assert_binary_snapshot!("golden_cyberpunk_theme.png", png);
}
