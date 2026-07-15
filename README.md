<p align="center">
  <img src="docs/images/sprite-banner.png" alt="Pocket Office characters" width="500" />
</p>

<h1 align="center">Pocket Office</h1>

<p align="center">
  <em>Your AI agents, visualized as pixel-art coworkers in a living terminal office.</em>
</p>

<p align="center">
  <a href="https://github.com/andyzhou0692-svg/Pocket-Office/stargazers"><img src="https://img.shields.io/github/stars/andyzhou0692-svg/Pocket-Office?style=flat-square" alt="Stars" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License" /></a>
  <a href="https://github.com/andyzhou0692-svg/Pocket-Office/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/andyzhou0692-svg/Pocket-Office/ci.yml?style=flat-square&label=CI" alt="CI" /></a>
</p>

<p align="center">
  <img src="docs/images/demo.gif" alt="Pocket Office animated demo" width="800" />
</p>

## What it is

Pocket Office turns real Claude Code and Codex sessions into coworkers in a small pixel office. Working agents type at their desks, waiting agents ask for attention, idle coworkers wander, chat and visit shared spaces.

It is a local ambient display, not another agent system. The office does not classify your work, call a model or spend tokens to decide what a character should do.

## Quick start

Build the current release from source:

<!-- install:start · generated from site/src/install.json by `just gen-readme` — edit the JSON, not this block -->
**From source** (macOS or Linux):

```bash
git clone https://github.com/andyzhou0692-svg/Pocket-Office.git
cd Pocket-Office
cargo build --release -p pixtuoid
mkdir -p "$HOME/.local/bin"
install -m 755 target/release/pixtuoid "$HOME/.local/bin/pocket-office"
```
<!-- install:end -->

Launch it:

```bash
pocket-office
```

Press `s` to connect Claude Code, Codex or another supported agent CLI. Start an agent in another terminal and its character will enter the office.

Useful controls: `q` quit · `p` pause · `s` sources · `t` themes · `Tab` agent dashboard · `?` help · `↑↓/jk/PgUp/PgDn` floors.

The always-on-top ambient window is available through:

```bash
pocket-office floating
```

## Features

<!-- features:start · generated from site/src/features.json by `just gen-readme` — edit the JSON, not this table -->
| | Feature | Description |
|---|---|---|
| 🏢 | **Multi-agent office** | Each agent session gets a desk; overflow agents auto-fill new floors |
| 🛗 | **Multi-floor office** | PageUp/PageDown/↑↓/jk to navigate floors with slide transition |
| 🪟 | **Floating desktop window** | `pocket-office floating` opens a frameless, always-on-top desktop window of the office, not just a terminal TUI |
| 🦞 | **OpenClaw gateway mascot** | A live OpenClaw gateway shows up as a wandering lobster whose motion tracks gateway health |
| 🎛️ | **Vibing** | A sun and moon arc the skyline as the day turns, weather rolls past the windows (rain, storm, snow, fog, overcast, windy, smog), and nine themes reskin the office |
| 🐾 | **Office pets** | A cat or dog (one per floor) roams desks, pantry, sofas; sleeps near idle agents. Click to pet — pixel-art hearts float up |
| 🗂️ | **Agent tree dashboard** | Tab opens a foldable tree of every floor's agents — badged by CLI, with activity tints and tool-call counts |
| 🧭 | **Office spaces** | Cubicles, a meeting lounge, and a pantry — the office is laid out in distinct furnished zones, not just a grid of identical desks |
| <img src="docs/images/pix-icons/walk.png" alt=""> | **Animated characters** | Typing, waiting (`?`), sleeping (z's), walking with A\*-routed pathfinding |
| <img src="docs/images/pix-icons/palette.png" alt=""> | **Team palette** | Shirt + pants colored by working directory (same repo → same color, a glanceable org-chart); hair/skin per agent. 16 curated outfits |
| <img src="docs/images/pix-icons/glow.png" alt=""> | **Per-tool monitor glow** | Edit = blue, Bash = orange, Read = cyan — scannable at a glance |
| <img src="docs/images/pix-icons/magnify.png" alt=""> | **Hover tooltips** | Hover an agent for session duration, tool-call count and active-time %; hover any furniture — desks, sofas, plants, vending machine, printer — for its name |
| <img src="docs/images/pix-icons/shield.png" alt=""> | **Hook-safe** | The shim always exits 0 — a stuck visualizer can never block your agent |
<!-- features:end -->

Pocket Office adds its own recurring visual coworkers, higher-detail character faces, a seven-person ambient baseline, richer office assets and location themes with distinct scenery and office life. `200West` includes Hudson traffic, occasional yachts and a suited paddleboard commuter. Tokyo Night, Succession and New York each use their own local movement and dialogue treatment.

## Supported tools

<!-- tools:start · generated from site/src/sources.json by `just gen-readme` — edit the JSON, not this table -->
| Tool | Runs on |
|---|---|
| [Claude Code](https://code.claude.com) | macOS · Linux · Windows\* |
| [Codex CLI](https://github.com/openai/codex) | macOS · Linux · Windows\* |

_Also supported: [Antigravity CLI](https://github.com/antiGravity-AI/antigravity-cli), [DeepSeek-Reasonix](https://github.com/esengine/DeepSeek-Reasonix), [CodeWhale](https://github.com/Hmbown/CodeWhale), [Copilot CLI](https://github.com/github/copilot-cli), [opencode](https://github.com/anomalyco/opencode), [Cursor CLI](https://cursor.com/cli), [Hermes Agent](https://hermes-agent.nousresearch.com), [Oh My Pi](https://omp.sh), [OpenClaw](https://github.com/openclaw/openclaw)._

**→ [Supported tools and setup notes](https://github.com/andyzhou0692-svg/Pocket-Office#supported-tools)**

_\* experimental — limited testing, unsigned binaries._
<!-- tools:end -->

## Configuration

Configuration currently lives at `~/.config/pixtuoid/config.toml`. This inherited path remains stable so existing users and hook integrations do not break. Every key is optional. See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for themes, desk capacity, persistent visual names, pets, custom sprite packs and furniture positions.

Examples:

```bash
pocket-office run --theme 200West
pocket-office run --theme tokyo-night
```

## How it works

Agent CLIs emit local lifecycle events through a hook shim or read-only transcript watching. A reducer turns those events into office state and the shared renderer paints the terminal, floating window and web surfaces.

Pocket Office retains the inherited internal Rust crate and binary names (`pixtuoid*`) for compatibility. The user-facing launch command is `pocket-office`.

## Privacy and security

Pocket Office is local-only and telemetry-free. It does not send session data to a server and does not make model calls. Read [SECURITY.md](SECURITY.md) for the exact trust boundaries and vulnerability reporting process.

## Contributing

Contributions are welcome, especially new themes, sprite and decoration polish and agent CLI adapters. Read [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) before opening a pull request.

## License

Pocket Office is available under the [MIT License](LICENSE).

## Origins

Pocket Office began as a fork of [Pixtuoid](https://github.com/IvanWng97/pixtuoid) and is independently developed and maintained by Andy Zhou.
