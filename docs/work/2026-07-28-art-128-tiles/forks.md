# Forks — art pipeline on 128 px tiles

_A choice I resolved, with what was rejected and why. A fork is mine; a [blocker](blockers.md) is
the user's._

## F1 — Where the tile footprint is authored, and where it is cached {#f1}
_2026-07-28 · resolved at plan time · **the corpus authors it, the leaf caches it**_

**Chosen.** `thing.span` in [`content/visual/things.rd`](../../../content/visual/things.rd) stays the
one place a footprint is *authored*. Remaster **reads** it and writes a resolved
`tiles: [w, h]` + `span` into the leaf `meta.json` as a clearly-labelled cache, regenerated every run.

**Why.** The span is already authored, already ratified as `frame_span` (u4, pow2 tiles) in
[`VARIABLES.md`](../../VARIABLES.md), and already drives the wire. A second authored copy in the
texture tree would be a second place to change it and a guaranteed drift — and `textures/` is
gitignored, so the copy would be the *unversioned* one. Caching it into the leaf keeps
[`tex_manifest.rs`](../../../server/edge/src/tex_manifest.rs) free of a DSL dependency: it already
walks leaves and reads `atlas.json`, so reading one more field there costs nothing.

**Rejected — author it in the leaf `meta.json`.** Puts the authored value in a gitignored tree and
splits the footprint from `thing.size`/`anchor`, which live in the corpus and must agree with it.

**Rejected — author it in the `type/subtype` registry `meta.json`** (design decision 6). That file is
the *kind* registry (name → `kind_id`, form → `variant_id`). Footprints vary per variant, not per
kind, so it is the wrong granularity.

**Rejected — have `tex_manifest.rs` read the corpus.** Makes the edge server depend on the DSL
corpus purely for a number the art pipeline already has in hand.

## F2 — Non-square footprints vs a square frame {#f2}
_2026-07-28 · resolved at plan time · **record `[w, h]`, derive the span**_

**Chosen.** Record the true footprint `[w, h]` (conifer `[1, 2]`) and derive
`span = next_pow2(max(w, h))`, giving the square `span · TILE_PX` (conifer → 256²).

**Why.** `frame_span` is a single pow2 value, so the *frame* is square and 1×2 and 2×2 both land in
256². But the footprint is not a rendering detail — occupancy and the thing spatial model care that a
conifer is 1×2, and collapsing to `2` on the way in destroys that permanently. Deriving is free;
un-collapsing is impossible.

## F3 — `--pad` on an atlas {#f3}
_2026-07-28 · resolved at plan time · **per-cell inset, driven by `atlas.json`**_

**Chosen.** When a leaf carries `atlas.json`, `pad_maps.py` insets **every cell** rather than the
canvas. Leaves without one keep today's canvas-edge behaviour.

**Why.** `--pad` shipped 2026-07-28 guarding the canvas edge, which is right for a sprite and wrong
for an atlas twice over: interior cell boundaries — the ones the sampler actually crosses — get no
guard at all, and shrinking the content ring pulls the whole sheet off its cell grid (a 1024 sheet
with a 1 px canvas ring is 1022 of content, which is not 8 × 128). The data model already agrees:
`atlas.json`'s `pad` is documented in [`tex_manifest.rs`](../../../server/edge/src/tex_manifest.rs)
as a **normalized per-cell inset** that the client trims from each cell's UV rect. So the client is
already expecting per-cell; only the writer is wrong.

**Rejected — skip padding on atlases.** Leaves the AA spill the flag exists to fix.

**Rejected — bake the guard into `generate_tile.py` only.** Ground sheets would be guarded and
hand-authored linked atlases would not.

## F4 — The old per-cell linked folders {#f4}
_2026-07-28 · **open — decide in [P4](todo.md), do not entrench meanwhile**_

`textures/biome-tile/default/blueprint/wall/1..16/` is the superseded per-cell split; the
[design](../../components/dev/textures/design/texture-layout/README.md) marks held-whole atlases ✅
and the old shape ⚠️ NOT YET. Whether this stream folds them or merely stops adding to them is a P4
call once the linked 4×4 grid is actually being emitted. Recorded now so the choice is deliberate
rather than accidental.
