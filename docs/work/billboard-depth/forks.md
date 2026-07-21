# Forks — billboard-depth

_Decision points + options + which we chose + why. Resolve before (or as) the phase lands._

---

## F1 · Where the billboard W × H comes from — 2026-07-21 (open)

`placeThing` builds a **square** (`l.size · SQUARE`). A conifer needs a real width × height (tiles). Options:

- **(a) A new content field `&thing.billboard w h`** (tiles) — explicit per-def billboard box, decoded into
  the layout. Cleanest + authoritative, but changes the `thing_layout` stride (stride-7 → stride-9) — a
  cross-component variable, so `docs/VARIABLES.md`/`TABLES.md` own it and every reader updates.
- **(b) Derive from existing fields** — e.g. width = `footprint.fw` (or `size`), height = a per-def
  multiple. No wire change, but conflates footprint (logical occupancy) with the billboard's visual height,
  and can't express "1-tile footprint, 3-tile-tall tree" without a new field anyway.
- **(c) Height-multiplier field only** — keep `size` for width, add one `&thing.height_tiles`. Minimal wire
  delta.

**Lean:** (a) if we want tall things done right (the billboard height is genuinely independent of footprint +
sprite `size`); (c) if we want the smallest change that unblocks conifers. Decide at P0 with the content
model in view (`docs/object-model.md`, the thing-layout stride).

## F2 · How the shadow pass respects thing-occlusion — 2026-07-21 (open)

Conifers must draw over shadows while shadows still land on bare ground. The shadow cast is a screen-space
pass; it needs to know where a thing occludes the ground point. Options:

- **(a) Sample `zdepth-world` in the cast shader** — the shadow fragment maps to its world pos anyway; sample
  the world composite there, and if a thing's depth is nearer than the ground, **discard** the shadow (the
  conifer, drawn in the cold composite, shows through). Keeps the single-pass cast; needs the world
  composite bound into the shadow shader in register.
- **(b) Reorder: bake shadows into the cold composite before things** — draw shadows onto the ground layer,
  then things on top. Naturally correct ordering, but the shadow cast is screen-space + per-frame while the
  cold composite is toroidal/baked — mixing the two tiers is exactly the rect-fighting the shadows design
  avoids ([shadows README](../shadows/README.md)).
- **(c) A display-time depth test** — composite world + shadow + things in one display pass ordered by
  `zdepth-world`. Most general (sets up the eventual lit consumer), most work now.

**Lean:** (a) for this stream — smallest change, keeps the screen-space cast, and `zdepth-world` (P1) is
exactly the input it needs. Revisit toward (c) when the lit shadow consumer replaces the direct display.
