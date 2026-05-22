//! Snapshot regression tests for `draw_scene`.
//!
//! Hashes the pixel buffer of a deterministic scene at a fixed `now` and
//! asserts the hash matches when the same input is rendered twice. Also
//! checks that the time-of-day system actually affects the render (so a
//! refactor that accidentally bypasses the daylight cycle is caught).
//!
//! What this DOESN'T cover: full visual regressions against a golden
//! pixel buffer. The daylight code reads chrono::Local, which makes a
//! golden hash machine-dependent. The determinism check + time-sensitivity
//! check together catch the most likely regressions (nondeterminism,
//! broken time wiring) without needing per-machine goldens.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use ascii_agents::tui::embedded_pack::load_default_pack;
use ascii_agents::tui::frame_cache::FrameCache;
use ascii_agents::tui::renderer::draw_scene;
use ascii_agents_core::source::Activity;
use ascii_agents_core::sprite::{Rgb, RgbBuffer};
use ascii_agents_core::state::ActivityState;
use ascii_agents_core::{AgentId, AgentSlot, SceneState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn fixture_scene(now: SystemTime) -> SceneState {
    let mut s = SceneState::new(12);
    let age_offset = Duration::from_secs(60);
    let cases = [
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
                source: "claude-code".into(),
                session_id: format!("session-{i}"),
                cwd: PathBuf::from("/demo"),
                label: key.to_string(),
                state: state.clone(),
                state_started_at: now,
                created_at,
                exiting_at: None,
                desk_index: i,
            },
        );
    }
    s
}

fn render_pixel_hash(now: SystemTime) -> u64 {
    let scene = fixture_scene(now);
    let backend = TestBackend::new(96, 36);
    let mut term = Terminal::new(backend).expect("terminal");
    let mut buf = RgbBuffer::filled(0, 0, Rgb(0, 0, 0));
    let pack = load_default_pack().expect("pack");
    let mut cache = FrameCache::new();
    draw_scene(&mut term, &scene, &pack, now, &mut buf, &mut cache).expect("render");

    let mut hasher = DefaultHasher::new();
    for px in &buf.pixels {
        px.0.hash(&mut hasher);
        px.1.hash(&mut hasher);
        px.2.hash(&mut hasher);
    }
    hasher.finish()
}

#[test]
fn render_is_deterministic_for_same_now() {
    // Anchor at a deterministic timestamp. Both runs use the same `now`,
    // so any randomness, hashmap iteration leak, or unstable Vec ordering
    // would show up as a hash mismatch here.
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_716_286_800);
    let hash_a = render_pixel_hash(now);
    let hash_b = render_pixel_hash(now);
    assert_eq!(
        hash_a, hash_b,
        "render is non-deterministic — repeat calls with identical input produce different output"
    );
}

#[test]
fn render_changes_when_time_advances_by_hours() {
    // Cross-check: the time-of-day system actually drives the output.
    // A refactor that accidentally hardcoded the daylight constants or
    // bypassed `now` in a paint pass would make these hashes identical.
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_716_286_800);
    let later = base + Duration::from_secs(8 * 3600);
    let hash_a = render_pixel_hash(base);
    let hash_b = render_pixel_hash(later);
    assert_ne!(
        hash_a, hash_b,
        "render is identical 8 hours apart — time-of-day system appears bypassed"
    );
}

#[test]
fn render_changes_when_an_agent_state_changes() {
    // Active vs all-idle should produce different pixels (screen glow,
    // active monitor scanline, skin tint differ).
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_716_286_800);
    let mut scene_idle = fixture_scene(now);
    for slot in scene_idle.agents.values_mut() {
        slot.state = ActivityState::Idle;
    }
    let backend = TestBackend::new(96, 36);
    let mut term = Terminal::new(backend).expect("terminal");
    let mut buf = RgbBuffer::filled(0, 0, Rgb(0, 0, 0));
    let pack = load_default_pack().expect("pack");
    let mut cache = FrameCache::new();
    draw_scene(&mut term, &scene_idle, &pack, now, &mut buf, &mut cache).expect("render");
    let mut hasher = DefaultHasher::new();
    for px in &buf.pixels {
        px.0.hash(&mut hasher);
        px.1.hash(&mut hasher);
        px.2.hash(&mut hasher);
    }
    let idle_hash = hasher.finish();

    let active_hash = render_pixel_hash(now);
    assert_ne!(
        idle_hash, active_hash,
        "all-idle and mixed-state scenes produced identical pixels"
    );
}
