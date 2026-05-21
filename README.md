# ascii-agents

A terminal-native, multi-agent pixel-art visualizer for AI coding agents. Each running session of [Claude Code](https://claude.com/claude-code) appears as an animated half-block sprite in a top-down ASCII office — sitting at a desk with a monitor that lights up green while typing, yellow when waiting for permission, dim cyan when idle.

![Top-down view of a pixel-art office with four characters at desks and one decorative plant](docs/images/screenshot.png)

> Inspired by [`pablodelucca/pixel-agents`](https://github.com/pablodelucca/pixel-agents) (VS Code webview) and [`rullerzhou-afk/clawd-on-desk`](https://github.com/rullerzhou-afk/clawd-on-desk) (desktop pet). Different niche: pure terminal, no Electron, no browser, runs over SSH.

## Status

**v0.1 (alpha) — pipeline works end-to-end against real Claude Code transcripts.**

| Working | Pending |
|---|---|
| Claude Code source (hook socket + JSONL fallback) | Codex / Cursor / Gemini source adapters (v2) |
| Multi-agent ASCII office, auto-assigned desks | Walking + BFS pathfinding (v2) |
| Idle / typing / waiting animation states | Reading / thinking states (v2) |
| Half-block pixel-art sprite rendering, 24-bit color | Daemon split / detachable viewer (v2) |
| Per-agent shirt + hair recolor by session-id hash | Office layout editor (v2) |
| `install-hooks` / `uninstall-hooks` (stow-symlink safe) | Web / GIF / OBS renderers (v2) |
| `--headless` mode for CI / scripting | |
| 46 tests + PNG snapshot of rendered TUI | |

## Install

Requires Rust 1.78+ (`brew install rust` on macOS).

```bash
git clone https://github.com/IvanWng97/ascii-agents
cd ascii-agents
cargo build --release
```

This produces two binaries:
- `target/release/ascii-agents` — the TUI
- `target/release/ascii-agents-hook` — the tiny shim CC invokes from its hooks

## Quick start

In one terminal:

```bash
# Wire Claude Code's hooks to our shim (writes to ~/.claude/settings.json
# atomically, with a one-time backup, and preserves a stow symlink if you use one).
./target/release/ascii-agents install-hooks \
    --hook-path "$(pwd)/target/release/ascii-agents-hook"

# Start the TUI.
./target/release/ascii-agents
```

In another terminal:

```bash
# Start a Claude Code session in any project.
claude
```

A character should appear at desk 0 within a second. When CC starts a tool call, the character switches to the typing animation. When it asks for permission, a yellow `┌─?─┐` speech bubble appears.

**`q` / Esc / Ctrl-C quits the TUI.**

Hooks stay installed across TUI sessions — you only run `install-hooks` once. When the TUI isn't running, the shim's socket connect fails silently and CC continues normally (zero overhead, the shim always exits 0). Run `ascii-agents` again any time you want to watch.

Only run `uninstall-hooks` if you want to **permanently remove** the integration from `~/.claude/settings.json`:

```bash
./target/release/ascii-agents uninstall-hooks
```

### Headless / scripting

```bash
./target/release/ascii-agents run --headless \
    --projects-root ~/.claude/projects --max-desks 12
```

Prints a one-line JSON-ish summary of scene state every time it changes. Useful for CI and for observing transcripts you're not actively viewing.

### CLI

```
ascii-agents [OPTIONS] [COMMAND]

Commands:
  run              Run the TUI (default if no subcommand given)
    --socket          override hook socket path (default /tmp/ascii-agents.sock)
    --projects-root   override CC transcript root (default ~/.claude/projects)
    --max-desks       desks per office (default 8)
    --headless        skip TUI; print scene state on changes
  install-hooks       merge our hooks into ~/.claude/settings.json
    --hook-path       path to ascii-agents-hook binary (else auto-detect)
    --settings        override settings.json path (defaults to ~/.claude/settings.json)
  uninstall-hooks     remove our hooks from settings.json
  help                Print help

Options:
  --log-level <LEVEL>    tracing filter (default: info)
```

## How it works

```
CC tool call ──► CC fires hook ──► ascii-agents-hook (shim)
                                         │ JSON line over Unix socket
                                         ▼
                                  /tmp/ascii-agents.sock
                                         │
                       HookSocketListener (core) ─────► ┐
                                                        │  (Transport, AgentEvent)
                       JsonlWatcher    (core) ─────► ───┤  on a shared mpsc channel
                                                        ▼
                       Reducer applies → Arc<RwLock<SceneState>>
                                                        │
                                                        ▼
                       TUI render loop @ ~30fps
                       (sprite frame → RgbBuffer → half-block cells → ratatui)
```

Hooks are primary (low latency, real-time permission events). JSONL transcript watching is the fallback for sessions where hooks aren't installed.

Three Rust crates:

- **`ascii-agents-core`** — headless library, no terminal dependencies. Owns the `Source` trait (for plugging in Codex / Cursor / Gemini later), the `Renderer` trait, the reducer, the sprite engine.
- **`ascii-agents`** — TUI binary built on ratatui + crossterm + tokio.
- **`ascii-agents-hook`** — tiny ~40-line shim CC invokes from its hooks. Forwards stdin JSON to the Unix socket with a 200 ms write timeout so a stuck daemon can never block CC.

## Verify visually

Build and render a snapshot without needing a real terminal:

```bash
cargo run --release --example snapshot -- /tmp/snap.png
open /tmp/snap.png      # macOS
```

## Multi-agent / extending

`Source` (in `crates/ascii-agents-core/src/source/mod.rs`) is the only abstraction required to add a new agent CLI:

```rust
#[async_trait]
pub trait Source: Send + 'static {
    fn name(&self) -> &str;
    async fn run(self: Box<Self>, tx: TaggedSender) -> anyhow::Result<()>;
}
```

A v2 `CodexSource` / `CursorSource` / `GeminiSource` plugs in by implementing the trait and writing tagged events onto the channel.

## Design + plan

- Spec: [`docs/superpowers/specs/2026-05-20-ascii-agents-design.md`](docs/superpowers/specs/2026-05-20-ascii-agents-design.md)
- Implementation plan (28 TDD-shaped tasks): [`docs/superpowers/plans/2026-05-20-ascii-agents-v1.md`](docs/superpowers/plans/2026-05-20-ascii-agents-v1.md)

## Known sharp edges

- **Hook payloads from CC don't include `tool_use_id`.** The reducer's hook-wins dedup window therefore rarely fires for the common case. Hooks always arrive ~ms before JSONL, so duplicate state transitions are benign in practice (state is re-set to the same value). A coarser per-session dedup is a candidate refinement.
- **The `Renderer` trait isn't on the TUI's live path yet** — `draw_scene` is generic over `Backend` (so `TestBackend` works for the snapshot example), but the production binary calls it as a free function rather than via a `Renderer` impl. Tracked for v2 daemon split.
- **`AgentSlot.state_started_at` is `std::time::Instant`** — process-local, not serializable. Will need to swap to `SystemTime` (or epoch-ms `u64`) before the daemon split.

## Acknowledgments

- [`pablodelucca/pixel-agents`](https://github.com/pablodelucca/pixel-agents) — the inspiration. Same concept, VS Code webview instead of terminal.
- [`rullerzhou-afk/clawd-on-desk`](https://github.com/rullerzhou-afk/clawd-on-desk) — multi-agent hook-based pattern, desktop-pet form factor.
- Claude Code's built-in [Buddy](https://dev.to/picklepixel/how-i-reverse-engineered-claude-codes-hidden-pet-system-8l7) ASCII pet — proves a single-character terminal pet idea is delightful; this project extends it to multi-agent + zoomed-out scene.

## License

MIT.
