# Completed — def frame/anchor rework

_Done **and** verified (overlay + identity diff). Items move here from [`todo.md`](todo.md).
Append-only history; authoritative for what's done._

---

## P0 · Layout ratified — 2026-07-23 ✓

- User ratified with **F1 as u4** (`frame_span` = tiles−1, width = `log2(ZONE_DIM)` so a zone-size
  bump drags it forward), **F4** signed `nudge_x` (+1024), and the arithmetic fixes (GREEN u12; ppu
  doubles per lod). `docs/VARIABLES.md` rewritten first — VARIABLES leads, code conforms.

## P1 · Pipeline invariants — 2026-07-23 ✓

- `LodPool.add` warns on non-pow2-square / <16px frames and off-16-grid placement. No warns fired
  live — the per-size-pool + pow2-master pipeline satisfies the invariants as-is.

## P2 + P3 · Encode/decode on the whole-px-per-unit model — 2026-07-23 ✓

- `definitionFor`: minimum bbox in EVEN units (top-left grid bias, rounded out), unsigned
  frame-relative `offset`, `frame_lod` from the pow2 side, `frame_span` u4 (pow2-rounded + capped
  at ZONE_DIM, warn), nudges at the resolved lod's px (x centered/signed, y bottom-aligned/unsigned,
  window clamped in-frame), anchors written bottom-center (1,2). ±512 bias + dilated-rect scheme
  deleted; `defTight` = the unit bbox × UNIT (1 frame unit ≡ 1 world unit by the span model).
- `casterCover`: `ppu = 2^lod / spanU` (whole pow2 by construction); anchor shift = bbox anchored
  point − full-box anchored point (billboard_data's base-centre keeps working — F3 option (b) for now,
  (a) lands with P5); `lod < 4` = solid-quad fallback; sample = frame origin + offset·ppu − nudge +
  normalized (s, 1−t)·bbox·ppu. Mirror (rot=W) flips the shift and s.

## P4 · Verified — 2026-07-23 ✓

- **Identity:** corridor↔brute **0 mismatches** at zoom 0.5 (9,949 nonzero texels, fully-streamed
  world) AND zoom 1 (8,329) — the walk is layout-agnostic, as designed.
- **Visual:** tapered conifer silhouettes + shrub crescents, bottom-aligned bases, radial
  directions correct at both zooms; loose (lod 0) defs degrade to solid quads during streaming and
  upgrade in place.
- **LOD swap live:** zoom 0.5→1 rewrote all 32 defs lod 5→6 (new frame origin/offset/nudge in one
  compare-write; tree bbox re-derived 16×30@(8,2)/32px → 18×30@(7,1)/64px) with no stale defs.
- Def decode (debugDef) reads meaningfully: tree = span 2, lod 6, anchor (1,2), nudge (0,0) at
  64px; shrub = span 1, ppu 4.

## P5 · span + sprite_scale from the DSL, scale-at-ingest — 2026-07-23 ✓

- **DSL** (`shared/dsl` loader): `VisualParts` += `span` (frame world span, pow2 tiles) +
  `sprite_scale (w, h)`; `thing_layout` stride 7 → **10**; wasm rebuilt. Content authored: conifer
  `span 2`; shrub/cactus/reed/rock/flora/wolf `sprite_scale 0.5/1.125`. Legacy `size` kept only for
  the pixijs client (stride bumped there; dies with pixijs).
- **Placement**: `placeThing` draws **span tiles**; a `white`/geo prim (never atlased) applies its
  sprite_scale at DRAW — the only stage its "sprite" exists. Real stems are exact span boxes.
- **Ingest** (`TextureResolver`/`LodPool`/`TextureAtlas`): a scaled stem packs scaled — clipped to
  the same pow2 frame (dest∩frame with matching source-UV narrowing — exact, no scissor),
  transparent-filled, **re-centred so the pre-scale surface-presence centre lands at the frame
  centre**; `spriteBBox` stores the post-transform fractions. Non-surface maps arriving before the
  surface defer one round (bytes cached). A scale CHANGE evicts the stem's packed LODs.
- Shadow side needed **zero changes**: span reaches `definitionFor` through `prim.width = span·SQUARE`
  and the def model re-derives everything from the (scaled) surface bbox.

## P6 · IMMUTABLE per-lod defs + the prim dirty cascade — 2026-07-23 ✓

- **One def per atlas frame**, keyed `(stem, cell, lod)` — lod 0 is the loose fallback. Nothing ever
  rewrites a def; a lod landing (or zoom retarget) mints a NEW def and `primDataFor` swaps the
  prim's `definition_index` in place, reporting `changed`.
- **The cascade**: a changed prim (def swap or first sight) dirties its own tiles + every light
  whose reach touches them + those lights' full cast regions (per-frame deduped). This replaced
  BOTH force-alls: the def-flush force-all AND the caster-count force-all (streaming-in cascades
  per prim; only caster REMOVAL still forces). The resolver onLoad → coldDirty hookup died with it.
- **Verified live**: defs `{lod0: 32, lod5: 32}` at zoom 0.5 and `{lod0: 32, lod6: 32}` at zoom 1 —
  the loose generation stays untouched while prims ride the cascade to the resolved generation;
  corridor↔brute **0 mismatches** both times; nonzero 9,949 = byte-identical to the pre-P5 settled
  field. Shrubs/rocks draw half-tile via scale-at-draw; silhouettes unchanged.
