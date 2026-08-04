# Art pipeline on 128 px tiles, sized by tile footprint — 2026-07-28

_Component: [`dev/art`](../../components/dev/) · `bin/art`, `bin/lib/{generate_tile,pad_maps,texpath}.py`
· surfaces through [`server/edge/src/tex_manifest.rs`](../../../server/edge/src/tex_manifest.rs).
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md).
Authority: [`VARIABLES.md`](../../VARIABLES.md) (`frame_span`, `frame_lod`) and
[`texture-layout`](../../components/dev/textures/design/texture-layout/README.md) — this stream
conforms the tooling to them, it does not redefine them._

## What the user asked for

**The game tile becomes 128 px**, and **every texture carries how many tiles it is**, so the packer
can place it. Textures stay pow2 squares. A conifer is 1×2 tiles → a 256² texture. Ground sheets
become 8×8 tiles = 1024². Linked kinds (`smooth/wall`, `smooth/rock`, `smooth/fence`) are atlases of
4×4 tiles = 512², and still need a tile size so they pack.

## This is conforming the tooling to a model that already exists

The sizing rule is **already ratified** and already authored — the gap is that the art pipeline is
the one thing that doesn't know about it.

| the concept | where it already lives | today's value |
|---|---|---|
| frame span in tiles | `thing.span` in [`content/visual/things.rd`](../../../content/things.toml) | conifer `2 &thing.span set` |
| the wire field | `frame_span` — u4, pow2 tiles (1/2/4/8/16), capped at one zone ([VARIABLES](../../VARIABLES.md)) | ✅ ratified |
| the derived square | `frame_span · SQUARE / 2^lod` ([VARIABLES](../../VARIABLES.md)) | max side `16 × 128 = 2048` |
| the atlas sidecar | `atlas.json` (`grid`, `pad`) → [`tex_manifest.rs`](../../../server/edge/src/tex_manifest.rs) → client UV | ✅ shipping |

So `frame_span · SQUARE` at `SQUARE = 128` **already yields the user's numbers**: span 2 → 256,
ground 8 → 1024, linked 4 → 512. Nothing here needs a new authority, and **this stream must not mint
one** — `thing.span` in the corpus stays the place a footprint is authored.

What is actually missing is that **`bin/art` sizes by guesswork**: `_emit_crops` fits each detected
blob to its own nearest pow2 box (`_pow2_box`), which is why a conifer variant came out `191×312 →
256×256` — right by luck, from blob extent, not from its declared span. Nothing in the pipeline ever
reads `span`. That is the hole this stream closes.

## Depends on `SQUARE = 128`, which is a different stream

[`2026-07-28-square-128`](../2026-07-28-square-128/README.md) (open) owns the renderer constant, and
owns it well — it splits `TEXTILE_LIGHT = 64` from `SQUARE` so the art gets 128 px **without** giving
back the 3.3× lighting win the `2b1025a` A/B bought. **Do not raise `SQUARE` from this stream.**

The boundary: `square-128` owns the constant and a `--size 128` re-master sweep (its P3). This stream
owns the art pipeline's *tile geometry* — what grid a sheet is cut on, what square a leaf is sized to,
where the footprint is recorded, and how the guard ring works on an atlas. Until `SQUARE` lands at
128, everything here is parameterised on it rather than blocked by it: the tooling should take the
tile edge as a value, so both streams can land in either order.

## Design stance

- **Derive, never re-author.** The pow2 square is `next_pow2(max(w,h) tiles × TILE_PX)`. The pipeline
  reads the footprint; it does not invent one, and it does not become a second place to change it.
- **A guess that has been right is still a guess.** `_pow2_box` must stop deciding size for anything
  that declares a span — but it stays the fallback for art that declares nothing, or the corpus
  becomes a hard dependency of the art tools.
- **The guard ring is per-cell on an atlas.** `--pad` (shipped 2026-07-28) guards the *canvas* edge.
  On an 8×8 or 4×4 atlas that leaves every interior cell boundary unguarded and shrinks the whole
  sheet off its grid. `atlas.json`'s `pad` is already defined as a *normalized per-cell inset* and the
  client already trims it, so the data model is right and only `pad_maps.py` is wrong. See [F3](forks.md).
- **Ground wraps as a SHEET, not per cell.** The client picks a cell by world position modulo the
  sheet, so adjacent world tiles get adjacent cells, which already join. Only the sheet's outer wrap
  is a seam. Per-cell toroidal quilting is *not* wanted and would cost detail for nothing.

## Future intent this plan must not trim

- **Non-square footprints are real.** A conifer is 1×2, not 2×2. `frame_span` is a single pow2 value
  (a square frame), so the square comes from the larger axis — but the *footprint* is what gameplay
  and occupancy care about ([thing spatial model](../../VARIABLES.md)). Record `[w, h]`, derive the
  span from it; do not collapse to one number on the way in.
- **`frame_lod`.** The square is `span · SQUARE / 2^lod`, so a lod-1 conifer is 128², not 256². The
  metadata must survive that being used later — record the footprint, not a baked pixel size.
- **Linked forms are held whole.** Per the design, a linked leaf is
  `biome-tile/<biome>/<material>/<form>/` + `atlas.json`, with **no** per-cell numeric folders. The
  tree still has the old `blueprint/wall/1..16/` shape (design marks it ⚠️ NOT YET). This stream
  should not entrench the old shape while touching it.
