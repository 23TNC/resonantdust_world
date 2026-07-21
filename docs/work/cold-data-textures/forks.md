# Forks — cold-data-textures

_Decision points + options + which we chose + why. F1–F2 are worth the user's input (they change the field
meanings / cross-component layout); F3–F5 have clear leans I can resolve._

---

## F1 · Coordinate space + z/radius units (resolve before P0) — 2026-07-21 (open, user)

`u16` x/y = 0..65535 px ≈ **1024 tiles ≈ 64 zones**; `u8` z = 255; `u8` radius = 255. Options:

- **(a) Zone-relative** — x/y are px WITHIN the light's zone (16 tiles = 1024px, fits `u16` with 6 spare bits
  of sub-px if wanted); the shader adds the zone origin. Radius in **tiles** (`u8` = 255 tiles, ample); z in a
  coarse unit (e.g. px÷4 → 0..1020, covers `Lz`=480) or also zone-relative. **Natural** — cold data is
  zone-scoped, and it never clips.
- **(b) World px** — simplest to reason about, but caps the world at ~1024 tiles and `Lz`/radius overflow `u8`
  as px (my debug `Lz`=480, radius=256 already do).

**Lean:** (a) zone-relative + radius-in-tiles + a coarse z. It's the only option that doesn't clip a real
world, and cold lights already belong to a zone. This is a `VARIABLES.md` decision (the field's meaning), so
confirm before P0.

## F2 · The atlas frame needs a PAGE identifier (resolve before P1) — 2026-07-21 (open, user)

`u10` frame x/y/w/h fit a **1024²** page, but the large-LOD `LodPool` uses **2048²**, and the layout carries
**no page/texture id** — the shader can't know which atlas page to sample for a given prim. Options:

- **(a) One page for shadow-casting sprites** — commit their surface LODs to a single 1024² page; `u10` is
  exact, no page id needed. Simplest; a soft cap on distinct caster sprites per page (fine at current counts —
  ~7 thing types).
- **(b) Page index in the layout** — spend some `reserved` bits on a page id + bind an atlas **texture array**
  (`sampler2DArray`); the shader indexes the layer. General, more setup, lifts the cap.

**Lean:** (a) now (the current casters share one 64px page anyway — `shadow-projection` P4 already assumes it),
graft (b) if distinct caster sprites outgrow a page. Note the cap in P1.

## F3 · Light ↔ LUT-entry association for the draw — 2026-07-21 (open)

The instanced draw needs each caster-instance to know its light, but a LUT ref stores no light id. Options:

- **(a) Per-light draws** — one instanced draw per light: `instanceCount = lut_count`, a `uLightIndex` uniform,
  read `LUT[lut_index + gl_InstanceID]`. 6 draws (one per light); trivially correct, matches the LUT's run
  shape.
- **(b) `light_index` in the LUT ref** — store the owning light in the ref's `u6` reserved (≤64 lights); one
  draw over all LUT entries, each reads its light. Fewer draws, spends the reserved bits.

**Lean:** (a) to land it (6 draws is nothing; keeps the LUT ref clean), revisit (b) if the draw count matters.

## F4 · The cold-light source — 2026-07-21 (open)

The lights are still **debug-seeded** (the 6-light ring). This stream is the TRANSPORT (lights → texture), not
the source. Real **DSL-authored cold lights** (zone content) are a separate, later source that fills the same
texture. **Lean:** keep the debug seed writing into the texture for now; DSL cold lights are out of scope
(they slot in behind the same `cold-light` layout when authored).

## F5 · LUT rebuild strategy on caster-set change — 2026-07-21 (open)

Contiguous per-light runs mean inserting a caster into light k's run shifts every later run. Options:
**(a) full rebuild** on any caster-set change (rare for cold — a zone streams in / a light moves); **(b)**
incremental patching (gap-buffer / free-list per light) later. **Lean:** (a) — cold data changes rarely, a full
rebuild is cheap and simple; incremental is premature.
