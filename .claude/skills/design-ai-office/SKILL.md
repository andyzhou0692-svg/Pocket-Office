---
name: design-ai-office
description: Use when translating a natural-language request about AIoffice or the Pixtuoid fork into a scoped visual change, including office graphics, character sprites and labels, furniture layout, avatar idle behavior or small talk, window scenery and weather, or thematic environments. Routes the request to the existing Pixtuoid skills and preserves the ambient, local, zero-token visualization boundary before implementation.
---

# Design AI Office

## Overview

Turn Andy's design language into the smallest coherent AIoffice change that produces the requested visible result. Own the scope and orchestration; reuse the existing Pixtuoid specialist skills for implementation and verification.

This is a visual product workflow. It must not change how the underlying agents think, route work, receive instructions, or consume model tokens.

## Fixed product contract

Preserve these decisions unless Andy explicitly changes them:

1. AIoffice is an ambient local visualization of real Claude and Codex activity.
2. Vivian is the real named root session. Real delegated subagents receive stable recurring visual names from a fixed local roster.
3. Other recurring office names are render-only residents. Names and titles carry no personality, skill, authority, or task routing.
4. Do not add semantic classification, keyword routing, hidden role markers, extra model calls, or synthetic agent sessions for visual effects.
5. The interactive terminal dashboard and passive floating display remain distinct surfaces.
6. Reuse current configuration, theme, scene, sprite, layout, and ambient-behavior systems before proposing a new subsystem.
7. Recommendations are not decisions. Do not implement an unchosen addition because it is convenient or interesting.
8. Render no more than seven idle avatars at once, including Vivian and persistent residents. Active and waiting agents remain visible.

## Recover the actual request

Before designing or editing:

1. Read the repository instructions exposed by the active harness. In this repo, `AGENTS.md` and `CLAUDE.md` resolve the same Pixtuoid source of truth.
2. Load the active AIoffice program from the Secondbrain vault when the harness exposes it. Resolve it by program name; never copy a machine-specific absolute path into this skill or the product.
3. Inspect the current Git state and the exact implementation path behind the visible behavior.
4. Separate three things explicitly:
   - **Decided:** what Andy directly requested or approved.
   - **Recommended:** an optional addition with a stated reason.
   - **Not included:** adjacent work that is outside the request.
5. If Andy already approved the fixed outcome, proceed. Do not ask him to reconfirm it.
6. Ask once only when a missing choice would materially change the visible result or create a new subsystem.

For a multi-part design request, state this compact contract before implementation:

```text
Outcome: <the visible result Andy asked for>
Use: <existing mechanism or specialist skill>
Change: <the files or visual systems that need work>
Not included: <adjacent ideas that remain unchosen>
Accept when: <observable checks at actual render scale>
```

## Route to the existing skills

Read and follow every matched specialist skill. Do not copy its detailed checklist here.

| Request | Route |
|---|---|
| Add a palette or conventional color theme | [`add-theme`](../add-theme/SKILL.md) |
| Add or rebuild a decoration, window element, furniture sprite, character sprite, or scene asset | [`beautify-decoration`](../beautify-decoration/SKILL.md) |
| Add support for another agent CLI | [`add-source`](../add-source/SKILL.md) |
| Review a completed branch or prepare it for merge | [`two-lens-review`](../two-lens-review/SKILL.md) |

Some requests cross more than one row. Use the minimum combination that covers the decided outcome.

### Thematic environments are more than palettes

Treat a request such as `200West`, Tokyo, Succession, or New York as the exact bundle Andy names. A bundle may include palette, window scenery, props, weather treatment, and authored ambient chatter.

Do not silently turn that bundle into a generalized theme engine. Use the existing palette, scene, sprite, and chatter mechanisms. A reusable theme engine is additional scope and requires Andy's decision.

## Apply the design rules by work type

### Graphics and resolution

1. Determine whether the problem is missing source detail, poor sprite proportions, half-block compression, scaling, or stretching. Do not call every blurry result a resolution problem.
2. Never invent missing visual detail without art direction or a source reference. Show the design direction first when the source pixels do not contain the requested information.
3. Preserve sprite aspect ratio at normal, resized, and full-screen terminal sizes. Do not stretch character width to fill the viewport.
4. Optimize for silhouette, proportion, and color identity at actual terminal scale. Follow `beautify-decoration` for snapshot, crop, self-critique, and live-binary rebuild.
5. Validate characters with a face crop and full-body crop, not only the full office screenshot.

### Office layout and movable objects

1. For a requested placement change, use existing layout positions or configuration overrides first.
2. Preserve walkability, approach points, z-order, and desk capacity.
3. Moving specific objects is a layout change. Building drag-and-drop editing, persistence UI, or a layout editor is a new subsystem and must not be inferred from that request.

### Avatar behavior and small talk

1. Implement ambient actions with local renderer state, timers, and authored text.
2. Keep named jokes or recurring characters visual only.
3. Theme chatter may be static authored lines, such as banker small talk for `200West`; it must not call an LLM or inspect the user's task semantics.
4. Use tests first for behavior, timing, selection, and state transitions. Verify the visible animation in the rendered office afterward.

### Window scenery and weather

1. Reuse the existing scene and theme hooks.
2. Treat day, night, sun, rain, and snow as visual states. Define how they interact with a theme before adding assets.
3. Keep rare scenic events, such as a yacht or suited paddleboard commuter, local and non-blocking. Test their eligibility and frequency separately from their art.
4. Confirm scenery remains legible without obscuring the office or becoming the dominant animation.

## Implementation discipline

1. Trace the current mechanism before proposing files. Cite the existing function, asset, configuration field, or renderer state that will be extended.
2. Use test-driven development for behavior or configuration changes.
3. Use the visual iteration loop for sprites and scene assets.
4. Keep config changes as config changes. Do not build UI for a value Andy only asked to adjust.
5. Keep a requested one-off scenic detail as a scoped asset or event. Do not build a content framework unless Andy chooses one.
6. Preserve unrelated user changes in the worktree.
7. Keep local commit and remote push as separate actions. Never push without explicit approval.

## Verification contract

Match verification to the change:

| Change | Required evidence |
|---|---|
| Palette | Theme tests, generated media, rendered office inspection |
| Sprite or scenery | Targeted tests, snapshot crop, actual-size inspection, live binary rebuild |
| Character proportions | Face and body crops at normal and full-screen sizes; no horizontal stretch |
| Layout | Scene tests, walkability overlay, resized-window render |
| Ambient behavior | Failing test first, passing state/timing tests, rendered behavior observation |
| Chatter | Selection tests, theme isolation, confirmation that no model or task classifier is called |
| Portability | Git tracks canonical files and relative discovery links; no user-specific path is stored |

Before calling the work complete:

1. Demonstrate the requested visible result.
2. State any visual compromise plainly.
3. Confirm no unchosen subsystem or token-consuming behavior was added.
4. Run the repository's required review skill before merge.

## Example

Request:

> In 200West, add occasional yachts and the suited paddleboard commuter outside the Hudson window. Keep weather visible and do not change agent behavior.

Correct translation:

```text
Outcome: Rare Hudson traffic appears behind the office while existing weather remains readable.
Use: beautify-decoration plus the existing scene and ambient-event mechanisms.
Change: Add two small scenery assets, extend local event selection, and layer them with weather.
Not included: New theme engine, semantic routing, agent behavior changes, or extra model calls.
Accept when: Day, night, rain, and snow renders remain legible; event frequency tests pass; cropped and full-office snapshots read clearly.
```

Then inspect the existing window renderer and event state, write the behavior test first, build the smallest change, and run the visual verification loop.
