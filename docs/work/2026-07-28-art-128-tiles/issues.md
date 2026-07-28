# Issues — art pipeline on 128 px tiles

_Problems hit, and what the evidence actually showed. Findings recorded at plan time are marked as
such — they were read out of the code, not measured under this stream._

## I1 — `bin/art` sizes textures by blob extent, not by declared span {#i1}
_2026-07-28 · read at plan time from `bin/art` + a conifer split_

`_emit_crops` fits each detected blob to its own nearest pow2 box via `_pow2_box`. Observed on a
conifer copy:

```
5/diffuse.e.0.png  191x312 -> 256x256
6/diffuse.e.0.png  170x266 -> 256x256
7/diffuse.e.0.png  173x290 -> 256x256
```

256² is the right answer — conifer is `2 &thing.span set`, so `span · SQUARE` at 128 is 256 — but the
pipeline reached it from **blob extent**, having never read `span`. A variant whose art happens to be
drawn small would silently land in 128² and pack wrong. Nothing in `bin/art` or `bin/lib/*.py`
references `span` or `frame_span`.

This is the hole the stream closes ([P2](todo.md)). Note the fallback still matters: art that
declares no span has nothing else to size from, so `_pow2_box` stays as the fallback rather than
being deleted.

## I2 — `--pad` guards the canvas edge, which is wrong for an atlas {#i2}
_2026-07-28 · read at plan time from `bin/lib/pad_maps.py` + `tex_manifest.rs`_

`pad_maps.py` shrinks the whole map's content by N px and replicates the edge into the ring. On an
8×8 ground sheet or a 4×4 linked atlas that is wrong twice:

- the **interior** cell boundaries — the ones a sampler crosses when it moves between cells — get no
  guard at all;
- the content shrinks off the grid: a 1024 sheet with a 1 px canvas ring holds 1022 px of content,
  which is not `8 × 128`.

The consumer already expects the opposite. `tex_manifest.rs` documents `pad` as *"the normalized
per-cell inset `[padU, padV]` (a fraction of the whole atlas) the client trims off each cell's UV
rect"*. So the client-side model is per-cell and correct; only the writer disagrees. Resolved as
[F3](forks.md#f3), built in [P3](todo.md).

## I3 — `squareMath.ts` comment contradicts the constant {#i3}
_2026-07-28 · read at plan time_

`LOD_LEVELS`' doc comment says *"`SQUARE` (128) down to 32 px"* while `SQUARE` is `64` two dozen lines
above. Left from the `2b1025a` halving. Harmless to the build, actively misleading to anyone reading
for the tile size — which is exactly what this stream and
[`square-128`](../2026-07-28-square-128/README.md) both make people do. Fixed in [P5](todo.md)
whichever value wins.

## I9 — PLAN DEFECT: `bin/art` has no tile-size arithmetic to parameterise {#i9}
_2026-07-28 · P1.1 · **the item's premise was false**_

P1.1 said "replace the hardcoded 64 arithmetic in `bin/art`". There is none. `grep -n '\b64\b'
bin/art` returns **nothing**, and the acceptance criterion was therefore satisfied before the item
was written.

`bin/art` is already tile-size agnostic by construction:

- **linked cell size is derived**, not assumed — `cell = atlas width / GRID_COLS` (`bin/art:145`), so
  "a 320² and a 640² atlas both yield the same 16-cell set";
- **`_pow2_box` works in pixels** — it takes a blob's pixel extent and rounds to the nearest pow2. It
  has no notion of a tile at all.

So the pipeline never converts tiles → px today. That conversion **first becomes necessary at P2**,
where `span · TILE_PX` decides the square. Introducing `TILE_PX` now would add a constant with no
reader, which is exactly the kind of speculative scaffolding that rots.

**Resolution:** `TILE_PX` lands in P2 beside its first consumer. P1.1 is closed as verified-no-op
rather than built. The rest of P1 (the `generate_tile.py` defaults) is real and unaffected.

## I8 — `generate_tile.py` writes an `atlas.json` the server cannot read {#i8}
_2026-07-28 · P0.4 · **live defect** — verified against `read_atlas_meta`_

There are **two incompatible `atlas.json` schemas** in the tree:

| written by | schema | manifest verdict |
|---|---|---|
| `bin/art` linked path | `{"cols":4,"rows":4,"padU":0.0,"padV":0.0}` | ✅ **served as an atlas** |
| `bin/lib/generate_tile.py` | `{"grid":[16,16],"tile":64,"pad":0,"cell":64,…}` | ❌ **ignored → single image** |

[`read_atlas_meta`](../../../server/edge/src/tex_manifest.rs) requires `cols`/`rows`/`padU`/`padV`
and every lookup is a `?`, so one missing key returns `None` and the leaf is "treated as an ordinary
single-image stem" (its own comment). `generate_tile.py` shares **not one key** with it.

Verified by running the parser's logic over every `atlas.json` in the tree:

```
SERVED as atlas            textures/biome-tile/default/smooth/wall/atlas.json
IGNORED -> single image    textures/biome-tile/default/stone/5200/atlas.json
```

**So every ground sheet `generate_tile.py` has ever produced is served as one flat image, not a cell
grid.** The client never learns the grid and cannot sample a cell by UV. This predates the stream and
would have gone unnoticed — the sheets *look* right on disk, and nothing errors.

Fixed by a new P2 item: emit the manifest's schema. The generator's extra provenance fields
(`seam_energy`, `feature_size`, `seed`, …) are worth keeping, and can be — the parser ignores unknown
keys. It is only the *required four* that must be present.

Note `padU`/`padV` are `0.0` on `smooth/wall` today, i.e. no cell guard at all, which is consistent
with [I2](#i2) and is what [P3](todo.md) exists to fix.

## I6 — PLAN DEFECT: the texture square derives from `span`, NOT from `footprint` {#i6}
_2026-07-28 · P0.2 · read from `shared/dsl/src/loader.rs` — **[F2](forks.md#f2) was wrong**_

`footprint` already exists and is **not** the field that sizes a texture. Both are in
[`VisualParts`](../../../shared/dsl/src/loader.rs) and they are independent:

| field | is | conifer | default |
|---|---|---|---|
| `footprint (w,h)` | the tiles the prim **OCCUPIES** — movement, hit-testing, z-row | **1 × 1** | `(1,1)` (`loader.rs:292`) |
| `span` | the sprite FRAME's world span, pow2 tiles; drawn box is `span × span` | **2** | `1.0` (`loader.rs:297`) |

The conifer is `footprint 1×1` with `span 2` — it *occupies* one tile and *draws over* two. So the
user's "a conifer is 1×2 tiles" is its **visual extent**, whose pow2 envelope is `span = 2` → 256².

[F2](forks.md#f2) as written said: record the footprint `[w,h]` and derive
`span = next_pow2(max(w,h))`. For the conifer that is `next_pow2(max(1,1)) = 1` → **128², wrong by a
factor of two.** Occupancy and frame extent are simply different quantities and one cannot be
derived from the other — a 3×2 warehouse could occupy six tiles and draw within a 4-tile frame, or a
1×1 lamp-post could draw over 4.

**Corrected:** the texture square is `span · TILE_PX`, full stop. `footprint` is a gameplay fact this
stream must not touch, and must not fold into the square. F2 rewritten accordingly.

## I7 — `span` is documented as pow2 but never validated {#i7}
_2026-07-28 · P0.2 · `loader.rs:295-297` vs [`VARIABLES.md`](../../VARIABLES.md)_

`VARIABLES.md` says of `frame_span`: *"Valid values are pow2 tiles (1/2/4/8/16) — required for
integer ppu; the writer rounds up + warns."* The loader does `read_f("prims.0.span", 1.0)` and
neither rounds nor warns — any float passes straight through to `thing_layout[7]` and into
`thingPlacement.ts`, where the drawn box becomes `span × span` tiles.

So a def authoring `span 1.5` silently gets a non-pow2 frame and breaks the integer px-per-unit the
whole def-frame-anchors model rests on. Nothing in the corpus does this today (only conifer authors
`span`, and it is 2), so this is latent rather than live — but this stream is about to make `span`
the thing that decides every texture's size, which turns a latent authoring hazard into a live one.

Not in scope to fix here; recorded so P2 can round-and-warn **on the art side** at minimum, and so
the loader gap is on the record.

## I5 — `span` is declared ONCE in the whole corpus; the fallback is the common case {#i5}
_2026-07-28 · P0.1 · measured — `grep -rn "thing.span set" content/`_

The plan assumed the corpus authors footprints. It authors exactly one:

| def | texture | `size` | `span` | square @128 |
|---|---|---|---|---|
| `tree` | `biome-thing/default/conifer` | 2 | **2** | **256** |
| `flora` | `biome-thing/default/flora` | 0.5 | — | — |
| `wolf` | `pawn/animal/wolf` | 1.125 | — | — |
| `shrub` `cactus` `reed` `rock` `torch` `torch_blue` | `white` (placeholder) | 0.5 | — | — |

**9 defs carry `size`, 1 carries `span`.** Six of the nine are on the `white` placeholder, so only
three name real art, and two of those three declare no span.

**What this changes.** [F1](forks.md#f1) (corpus authors, leaf caches) still holds as the *shape* —
there must not be a second authored copy. But P2's "fall back to `_pow2_box` when absent" is not an
edge case, it is what will run for almost everything on day one. So:

- the fallback must be **good**, not a token branch, and must warn loudly enough to drive authoring;
- P2 should emit the *inferred* span into the leaf so there is a worklist of defs to go author;
- the wolf is the sharp case — `size 1.125` needs a 2-tile span, and it declares none, so whatever
  `frame_span` defaults to when absent decides whether the wolf currently clips. P0.2 must pin that
  default down.

## I4 — `thing.size` and `thing.span` are different numbers {#i4}
_2026-07-28 · read at plan time from `content/visual/things.rd`_

Both are "in tiles" and they are **not** the same field:

- `thing.size` — the sprite's draw scale in tiles. Conifer `2`, wolf `1.125`. Fractional.
- `thing.span` — the frame's pow2 tile span, i.e. what the texture square derives from. Conifer `2`.

The wolf makes the distinction concrete: `size 1.125` is not pow2 and cannot be a span, so its frame
must span 2 tiles while it draws at 1.125. **Size in tiles ≠ the square it packs into**, and any code
here must take `span` (or the footprint), never `size`.
