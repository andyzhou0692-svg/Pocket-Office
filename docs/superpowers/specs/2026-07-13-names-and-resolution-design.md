# Pixtuoid Names and Resolution Design

## Goal

Make two focused improvements to the existing Pixtuoid office:

1. Allow visual agent names to be configured.
2. Replace the current low detail character art with higher detail pixel art.

Pixtuoid remains a visualization. Agent behavior, voice and judgment continue to come from the existing vault and agent harness.

## Scope

### Configurable visual identities

Add an optional visual name table to Pixtuoid configuration. Each entry matches an existing Pixtuoid agent label and replaces it with a display name. The initial local configuration will contain:

1. Vivian
2. Tom
3. Amy
4. Jess

This is a direct visual alias only. Pixtuoid does not inspect prompts, skills or roles and does not change how agents are identified.

### Higher detail pixel art

Redraw the existing character sprite set at a detailed scale while keeping the current office structure and movement model. Character canvases increase from the current 8 pixel width to 12 pixels. The first pass covers seated, typing, standing, walking, coffee and sleeping character poses.

The terminal and floating painters continue to consume the same shared scene. No new renderer, window controls, camera system or office layout system will be introduced.

## Components

1. Configuration parser: reads optional visual name aliases.
2. Scene alias resolver: matches the raw label to a configured display name with the current label as fallback.
3. Sprite assets and dimensions: replace the character art and update only the size constants required by the larger assets.
4. Existing painters: render the updated scene without new interaction behavior.

## Data Flow

The existing source decoder produces a raw agent label. Pixtuoid matches that label against local visual name rules, stores the resolved display fields in scene state and renders them above the existing character. Unknown labels follow the current naming path.

## Compatibility and Errors

Existing configurations remain valid because visual name aliases are optional. Unknown labels do not stop the dashboard. They fall back to the existing label.

The higher detail sprites must preserve collision behavior and remain usable in both terminal and floating output. If the current terminal is too small for the detailed assets, Pixtuoid shows its existing size guidance rather than a blank scene.

## Verification

1. Configuration tests cover visual aliases.
2. Alias resolution tests cover known and unknown labels.
3. Scene tests verify updated sprite bounds and unchanged collision behavior.
4. Rendering checks cover terminal and floating output.
5. Existing test suites must continue to pass.

## Acceptance Criteria

1. Vivian, Tom, Amy and Jess can be renamed through configuration without editing Rust code.
2. Configured names appear in both terminal and floating labels.
3. An unconfigured session looks exactly as it does in upstream Pixtuoid.
4. Characters use 12 pixel wide detailed sprites in all core work and movement poses.
5. The existing office layout, controls, hooks, agent behavior and pet count remain unchanged.

## Explicitly Out of Scope

1. Personalities or prompts in Pixtuoid
2. New agent routing or vault behavior
3. Team pods, office hierarchy mechanics or new layouts
4. New floating window controls
5. Pet count changes
6. A native high resolution application or renderer rewrite
7. Automatic skill detection, role inference or subagent hierarchy labels
