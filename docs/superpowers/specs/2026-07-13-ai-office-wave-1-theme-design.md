# AI Office Wave 1 Theme Design

## Goal

Turn Pixtuoid themes from palette swaps into recognizable office worlds while preserving the current layout, movement model and lightweight runtime.

Goldman Sachs is the first production reference. It establishes the quality bar and the theme vocabulary that Tokyo Night, Succession and New York will later reuse.

## Approved Direction

The Goldman theme is a working investment bank floor, not a cinematic executive suite.

Andy supplied a reference image on 2026-07-13 showing a bright, dense workstation floor with long sightlines, repeated pale wood desks, black task chairs, blue monitor screens, desk phones and large ceiling light panels. That operational rhythm is the visual anchor.

The existing office layout stays intact. Theme identity comes from controlled substitutions to wardrobe, palette, materials, scenery, props, lighting and dialogue content.

## Current Theme Audit

| Theme | Current palette | Current materials | Current scenery | Current atmosphere | Current dialogue |
|---|---|---|---|---|---|
| Coworking | Warm brown, terracotta and blue | Warm wood, fabric and casual rug | Generic procedural city | Daylight lounge | Shared developer humor |
| Cyberpunk | Magenta, cyan and violet | Dark synthetic surfaces | Same generic city | Electric neon night | Shared developer humor |
| Dracula | Plum, pink and charcoal | Dark muted furniture | Same generic city | Soft gothic night | Shared developer humor |
| Tokyo Night | Navy, blue and lavender | Blue tinted generic furniture | Same generic city, no Tokyo landmark | Cool calm night | Shared developer humor |
| Catppuccin | Mocha, lavender and pastel pink | Soft muted furniture | Same generic city | Cozy pastel night | Shared developer humor |
| Gruvbox | Charcoal, amber and orange | Retro dark wood | Same generic city | Nostalgic terminal warmth | Shared developer humor |

The current `Theme` model controls colors. The skyline geometry, character wardrobe, decor assets and chitchat pool are shared across every theme. A complete location theme therefore needs more than a new color definition.

## Wave 1 Theme Matrix

| Theme | Palette | Office materials | Skyline or landmark | Atmosphere | Dialogue tone |
|---|---|---|---|---|---|
| Goldman Sachs | Midnight navy, charcoal, warm white and restrained Goldman blue | Pale maple, brushed metal, black mesh, cream stone and dark monitor bands | Hudson River, Jersey skyline and ferry lights | Bright operating floor by day, warm task light late at night | Deals, markets, analyst pressure and dry senior banker lines |
| Tokyo Night | Deep indigo, electric blue and tower red | Dark wood, black steel and clear glass | Tokyo Tower | Rain gloss, dense night lights and calm precision | Concise, focused and quietly intense |
| Succession | Ivory, black, cognac and muted gold | Limestone, smoked glass, leather and walnut | Central Park | Cold wealth, controlled daylight and dramatic sunset | Power, leverage, status and clipped contempt |
| New York | Steel blue, brick, cream and restrained taxi yellow | Terrazzo, black steel, brick and worn wood | Empire State Building | Fast city rhythm, changing weather and traffic glow | Sharp, direct, ambitious and local |

## Goldman Production Reference

### Visual read

A stranger should identify the scene as a Manhattan investment bank before reading the theme name.

The strongest cues are:

1. Repeated banker workstations
2. Dark suits against a bright neutral floor
3. Blue monitor screens and dense desk equipment
4. Long, orderly sightlines
5. A Hudson River view

Branding is supporting evidence, not the main device.

### Characters

Every human wears investment banking business attire.

1. Navy, charcoal and near black suits
2. White and pale blue shirts
3. Restrained ties, pocket squares or blouse accents for differentiation
4. Tailored trouser and skirt suit silhouettes
5. Existing individual skin and hair variation remains
6. No casual shirt palette in the Goldman theme

At terminal scale, jacket shape and shirt contrast matter more than lapel detail. The silhouette must read as a suit before small accessories are added.

### Office materials

The reference image replaces the earlier dark walnut executive office direction.

1. Pale maple or beech desks and cabinetry
2. Black mesh task chairs
3. Warm white or light beige walls
4. Brushed metal trim
5. Charcoal monitor bands and equipment
6. Blue grey low pile carpet
7. Bright rectangular ceiling light panels
8. Clear or lightly smoked meeting room glass

The result should feel expensive because it is precise, durable and ordered, not because it is ornate.

### Same layout, Goldman substitutions

The current room geometry and paths remain unchanged.

1. The main desk area becomes the dense banking floor through repeated pale desk surfaces, black chair backs, phones and blue screens.
2. The meeting room uses clear glass, a pale conference table and minimal finance presentation material.
3. The pantry becomes a clean institutional coffee station in brushed metal, white and black.
4. The lounge keeps its footprint but becomes a restrained client waiting area rather than a casual coworking lounge.
5. Plants stay sparse and architectural.

No new layout system, room type or navigation behavior is part of this lane.

### Workstation props

The first visual prop set should prioritize objects that read clearly at half block scale:

1. Desk phones
2. Dual or repeated blue monitors
3. Pitchbook stacks
4. Printed pages
5. Binders
6. Deal tombstones represented as simple bright blocks
7. An Eastern Time wall clock
8. One restrained `200 WEST` or blue brand sign

Props must preserve existing furniture footprints. Tiny decorative clutter that cannot be identified at normal terminal scale should be omitted.

### Window scenery

The Hudson River is the fixed Goldman view.

1. Give the river enough vertical depth to read as water rather than sky or empty glass.
2. Place a low Jersey skyline beyond the water.
3. Use horizontal reflections to distinguish the river from the sky.
4. Add sparse ferry lights or a wake as ambient motion if the shared scenery contract supports it.
5. Preserve time of day and weather behavior, including rain, fog, sunset and night lights.
6. Do not randomly substitute the Empire State Building or Central Park into Goldman. Those belong to other themes.

### Lighting and atmosphere

The Goldman theme should change character through the day:

1. Early morning uses cool clean daylight and quiet blue screens.
2. Market hours use bright ceiling panels, active screens and crisp neutral surfaces.
3. Sunset brings warm Hudson reflections into the office.
4. Late night reduces the ceiling wash and emphasizes monitor light and isolated task lighting.
5. Rain and fog should make the window wall feel especially Manhattan without obscuring the office.

### Dialogue content needs

Andy supplies the canonical Wall Street lines. Theme Design defines the content categories but does not implement the dialogue engine.

The Goldman pool should cover:

1. Deal urgency
2. Market shorthand
3. Analyst workload
4. Senior banker pressure
5. Client management
6. Dry finance humor
7. Late night office culture

The current bubbles favor very short lines. Avatar Behavior must decide how longer quotations are abbreviated, wrapped, paged or displayed before the final content pool is implemented.

## Recommendations

1. Build Goldman as the first complete vertical reference before expanding the portfolio.
2. Define a theme as palette plus wardrobe plus materials plus scenery plus props plus dialogue.
3. Keep each location theme tied to one primary view so the window becomes part of its identity.
4. Preserve tool glow and source badge meaning across every visual theme.
5. Use theme specific assets only when they preserve existing footprints and movement paths.
6. Apply the stranger identification test at normal terminal scale before accepting visual detail.
7. Use one restrained brand marker. Avoid wallpapering the office with logos or blue.
8. Deepen Tokyo Night next because it already has the strongest palette foundation and a clear missing landmark.

## Lane Boundaries

### Theme Design owns

1. Theme matrix
2. Goldman art direction
3. Palette, material, scenery and atmosphere recommendations
4. Dialogue tone and content requirements

### Visual Foundation must establish before implementation

1. How a theme selects an outfit sprite family
2. How a theme selects a skyline or landmark profile
3. How a theme selects decor variants without changing footprints
4. How themed material assets coexist with the current color fields

Theme Design does not edit shared rendering code before that contract exists.

### Avatar Behavior owns

1. Theme specific dialogue pools
2. Bubble length policy
3. Quote selection and timing behavior
4. The final Wall Street lines supplied by Andy

Theme Design does not implement that engine.

## Acceptance Criteria

1. Goldman is the first production reference.
2. The existing office layout and navigation remain unchanged.
3. Every human visibly reads as a banker in a suit.
4. The office reads as a bright working Goldman floor rather than a generic luxury office.
5. Pale desks, black chairs, blue screens, phones and ceiling panels reproduce the supplied reference vibe.
6. The windows show a legible Hudson River and Jersey skyline.
7. Time of day and weather remain visible.
8. Finance props remain identifiable at normal terminal scale.
9. Dialogue requirements are defined without implementing the Avatar Behavior engine.
10. No shared rendering change begins until Visual Foundation establishes the theme asset contract.
