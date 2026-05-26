# Pantry Chitchat — Design Spec

## Overview

When 2+ idle agents visit the same waypoint (Pantry, Couch, VendingMachine, Printer), they play a chitchat animation — alternating speech bubbles with funny dev-humor snippets over ~6 seconds.

## Trigger

- Two or more agents at the same waypoint index with `supports_chitchat(kind)` = true
- Detected in the pixel painter's `wp_rank` loop (already has pose + anchor + wp index)
- One conversation per waypoint per floor; keyed by `(floor_idx, wp_idx)`
- Conversations auto-expire after `CHITCHAT_TOTAL_MS` (6s)

## Conversation Flow

- 4 turns of 1.5s each = 6s total
- Turn 0, 2: Agent A speaks (smaller `agent_id.raw()` = stable speaker A)
- Turn 1, 3: Agent B speaks
- Each turn shows a random line from `CHITCHAT_LINES` (24 dev-humor one-liners)
- Seed: `agent_a.raw() * 0x9e37... ^ agent_b.raw() ^ started_at_ms` — same pair gets different conversations each meeting

## Speech Bubble Rendering

- Ratatui `Paragraph` widget overlay (not pixel buffer — text must be readable)
- Positioned above the speaking agent's sprite: `cell_y = anchor.y / 2 - 3`
- Width: `text.len() + 4`, height: 3 (border + text + border)
- Style: `tooltip_bg` background, white text
- Clipped via `clip_widget_rect` to avoid edge overflow

## Snippet Pool (24 entries)

```
"git push -f"  "// TODO"  "LGTM!"  "works on my"  "ship it!"  "npm install"
"sudo !!"  "404"  "seg fault"  "it compiled!"  "rebase time"  "merge pls"
"async await"  "rm -rf node_"  "NaN === NaN"  "overflow"  "undefined?"  "coffee++"
"looks good"  "trust me"  "¯\_(ツ)_/¯"  "🐛→🔨"  "🚀✨"  "☕→💡"
```

## Architecture

- `chitchat.rs` — `ActiveChitchat`, `ChitchatBubble`, `update_and_collect()`, snippet pool
- `TuiRenderer` owns `HashMap<(usize, usize), ActiveChitchat>` (persistent state)
- `render_to_rgb_buffer` returns `PixelPassResult { cat_pos, chitchat_bubbles }`
- `draw_scene` renders bubbles via `paint_chitchat_bubbles` in the `term.draw` closure
