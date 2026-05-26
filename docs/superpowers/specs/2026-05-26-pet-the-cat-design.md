# Pet the Cat — Design Spec

## Overview

Click the roaming office cat to trigger a state-dependent reaction with pixel-art heart particles. The cat pauses for 2 seconds, then resumes its normal wander cycle. Visual-only — no sound.

## Interaction Model

| Cat state | Click reaction | Tooltip on hover |
|---|---|---|
| Walking | Cat stops, sits down, hearts float up | "Office Cat (walking)" |
| Sitting | Hearts float up (already sitting) | "Pet me!" |
| Sleeping | Cat briefly wakes (sits), hearts float, goes back to sleep | "Shhh... sleeping" |
| Already petted (cooldown) | No effect | "purr..." |

## Heart Particle Effect

- 3–4 tiny 2×2 pixel hearts, same rendering technique as `paint_sleep_z`
- Color: `Rgb(255, 100, 100)` — warm red, distinct from any existing palette key
- Float upward from cat position, staggered timing (200ms apart)
- Each heart rises 6–8 pixels over 2 seconds, fading via alpha blend toward floor color
- Rendered in `effects.rs` alongside existing particle effects

## State Management

- New `CatPetState` struct stored on `TuiRenderer`:
  ```rust
  struct CatPetState {
      petted_at: Option<SystemTime>,
      pet_pos: Point,
      cat_anim: &'static str,  // "cat_sit" / "cat_sleep" before petting
  }
  ```
- When `petted_at` is within 2s of `now`:
  - Cat position freezes at `pet_pos`
  - Cat sprite overrides to `cat_sit` (regardless of previous state)
  - Heart particles render above the cat
- After 2s: `petted_at` clears, cat resumes normal `cat_position()` cycle
- Cooldown: clicks are ignored while `petted_at` is active (prevents spam)

## Hit-Test

- `hit_test_cat(layout, cat_pos, cat_anim, mx, my) -> bool` in `hit_test.rs`
- Bounding box depends on current sprite:
  - `cat_walk`: 8×6 px
  - `cat_sit`: 6×6 px
  - `cat_sleep`: 6×4 px
- Converts terminal cell coords to pixel coords (`py = my * 2`) like other hit-test functions
- Uses cat's current screen position (from `cat_position()` or frozen `pet_pos`)

## Hover Tooltip

- State-dependent text rendered via `paint_cat_tooltip` in `widgets.rs`
- Same style as furniture tooltips (dark bg, light text, positioned near cursor)
- Priority chain in `draw_scene`: agent tooltip > coffee machine > **cat** > furniture

## Files Changed

| File | Change |
|---|---|
| `tui/tui_renderer.rs` | Add `CatPetState` field, pass to draw_scene via DrawCtx |
| `tui/renderer.rs` | Add `cat_pet` field to `DrawCtx`, wire tooltip + click into draw closure |
| `tui/hit_test.rs` | Add `hit_test_cat` function |
| `tui/widgets.rs` | Add `paint_cat_tooltip` (state-dependent text) |
| `pixel_painter/effects.rs` | Add `paint_pet_hearts` (2×2 pixel hearts floating upward) |
| `pixel_painter/drawable.rs` | Check `CatPetState` — freeze position + override sprite when petted |
| `pixel_painter/mod.rs` | Thread `CatPetState` through to drawable cat rendering |
| `tui/mod.rs` | Wire click handler: after coffee machine, before agent pin |

## Architecture Notes

- Cat pet state lives on `TuiRenderer` (render-side only), not on `SceneState` — petting is a local visual effect, not a data model concern. Same pattern as `mouse_pos` and `pinned_agent`.
- The heart effect is purely a paint-time computation from `petted_at` + `now` — no frame-by-frame state accumulation needed.
- The cat's wander cycle (`cat_position()`) is wallclock-driven. Freezing the cat for 2s means the cycle advances without the cat — when petting ends, the cat picks up wherever the cycle says it should be (may teleport slightly). This is acceptable; the cat already "teleports" between wander destinations.

## Testing

- Unit test: `hit_test_cat` returns true for coords inside cat sprite, false outside
- Unit test: heart particle positions are correct at t=0, t=1s, t=2s
- Integration: cat tooltip shows correct text for each state (via widget_cells pattern)
