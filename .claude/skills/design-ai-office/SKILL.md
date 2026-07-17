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
2. Vivian is the real named root session. Tom, Jess, Amy and Alison are persistent residents that may become the display identities of real delegated subagents for the task lifetime.
3. Resident names and titles carry no personality, skill, authority or control over the real task. Overflow subagents receive recurring visual names from the local roster.
4. Display-only local keyword preferences may use metadata already held in memory, with a local fallback when no rule matches. Do not read task transcripts, add model calls, change real task routing, add hidden role markers or create synthetic agent sessions for visual effects.
5. The interactive terminal dashboard and passive floating display remain distinct surfaces.
6. Reuse current configuration, theme, scene, sprite, layout, and ambient-behavior systems before proposing a new subsystem.
7. Recommendations are not decisions. Do not implement an unchosen addition because it is convenient or interesting.
8. Keep at least eight avatars visible. Active and waiting agents occupy those eight first; display-only idle coworkers fill only the remaining seats. Above eight working agents, show no idle fillers. Never cap active or waiting agents.

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

1. Classify the failure before editing art: source detail, sprite proportions, shared RGB composition, target-painter flush, terminal glyph rasterization, scaling, stretching, or stale installation. Do not call every blurry or broken result a resolution problem.
2. Use cross-object correlation as the first diagnostic. If unrelated objects such as faces, desks, chairs, and walls share the same bands, gaps, stretching, or color leaks, treat the shared renderer or target painter as the leading cause until evidence disproves it. Do not redraw each asset.
3. Trace the same visible sample through four boundaries: source sprite, shared RGB buffer, target painter output, and the user's actual terminal or window. The first boundary where the defect appears owns the investigation.
4. Treat a clean synthetic render plus a broken native render as a hard stop on sprite editing. Inspect the terminal flush, half-block mapping, font glyph metrics, cell geometry, and installed binary before changing another asset.
5. Keep output corruption separate from source resolution. More sprite pixels can add detail after the target renders correctly; they cannot repair broken glyph rasterization or cell mapping.
6. After two rejected corrections with the same visible symptom, stop surface patching and return to systematic root-cause investigation before a third attempt.
7. Never invent missing visual detail without art direction or a source reference. Show the design direction first when the source pixels do not contain the requested information.
8. Preserve sprite aspect ratio at normal, resized, and full-screen terminal sizes. Do not stretch character width to fill the viewport.
9. Optimize for silhouette, proportion, and color identity at actual terminal scale. Follow `beautify-decoration` for boundary diagnosis, snapshot iteration, real-terminal proof, self-critique, and live-binary rebuild.
10. Validate characters with a face crop and full-body crop, not only the full office screenshot.
11. Treat TestBackend PNGs and GIFs as iteration evidence only. They do not reproduce the user's terminal font, glyph rasterization, cell proportions, or exact layout geometry.
12. For every TUI-visible graphics change, capture the freshly rebuilt proof binary inside the user's actual terminal application at the exact grid size that reproduced the issue. The office itself must be visible; footer-only output is a failed proof.
13. Keep the native terminal capture unscaled. A crop may support diagnosis, but it may not replace the full native capture or be enlarged and relabeled as terminal proof.

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
| Palette | Theme tests and generated media for iteration; native terminal capture through the user's actual launcher for acceptance |
| Sprite, scenery, or terminal rendering | Targeted tests and synthetic crops for iteration; freshly rebuilt proof-binary hash; full native capture from the user's actual terminal at the issue-reproducing grid; live launcher hash matches the verified build |
| Character proportions | Face and body crops plus full native terminal captures at normal and full-screen sizes; no horizontal stretch |
| Layout | Scene tests and walkability overlay; native terminal captures through the user's actual launcher at each affected grid size |
| Ambient behavior | Failing test first, passing state/timing tests, rendered behavior observation |
| Chatter | Selection tests, theme isolation, confirmation that no model or task classifier is called |
| Portability | Git tracks canonical files and relative discovery links; no user-specific path is stored |

Before calling the work complete:

1. Demonstrate the requested visible result.
2. For terminal-painter changes, test a two-color vertical pixel pair so the logical top and bottom colors are proven through the exact half-block symbol, foreground, and background mapping.
3. Confirm the proof used current source by recording the absolute proof-binary path and SHA256 after the final edit. Any later source or asset edit invalidates that proof.
4. Resolve the user's real launch command, verify its executable hashes to the rebuilt live binary, then run that exact launcher at the issue-reproducing grid and capture it natively. Do not assume `pixtuoid`, `pocket-office`, and a build artifact point to the same executable.
5. State any visual compromise plainly.
6. Confirm no unchosen subsystem or token-consuming behavior was added.
7. Run the repository's required review skill before merge.

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
