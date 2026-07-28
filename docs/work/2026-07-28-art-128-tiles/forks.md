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

## F2 — What the texture square derives from {#f2}
_2026-07-28 · **REWRITTEN at P0.2** — the original was wrong, see [I6](issues.md#i6)_

**Chosen.** The square is **`span · TILE_PX`**. Nothing else. `footprint` is not an input.

**Why.** `footprint (w,h)` and `span` already exist as separate fields and mean different things:
footprint is the tiles a prim *occupies* (movement, hit-testing, z-row), span is the sprite frame's
*world extent* in pow2 tiles. The conifer is `footprint 1×1`, `span 2` — it occupies one tile and
draws over two. They are independent quantities; a wide building could occupy 3×2 and draw within a
4-tile frame.

**What the original said, and why it was wrong.** It proposed recording `[w,h]` and deriving
`span = next_pow2(max(w,h))`. Applied to the conifer's real footprint of 1×1 that yields span 1 →
**128², half the correct size.** It read the user's "conifer is 1×2 tiles" as a footprint when it is
a visual extent. Caught at P0.2 by reading `loader.rs` rather than assuming.

**Consequence for P2.** The leaf caches `span` (and the resulting square). It may *also* carry
`footprint` for the manifest's convenience, but as a passenger — the art pipeline must never size
anything from it.

## F5 — What the art pipeline does when a def declares no span {#f5}
_2026-07-28 · resolved at P0.2 · **infer, warn, and emit a worklist**_

**Chosen.** Absent `span`, infer one from the art's opaque bbox
(`span = next_pow2(ceil(max(w,h) / TILE_PX))`), emit it into the leaf marked `"span_inferred": true`,
and warn naming the def. Do **not** fall back silently, and do **not** fail.

**Why.** [I5](issues.md#i5) measured that exactly one def in the corpus authors `span`, so a
"fallback" is what runs for almost everything — a token branch would mean almost every texture is
sized by guesswork with no record of it. Marking the inference makes the leaves self-describing and
gives P4 a worklist of defs to go author. Failing instead would make the DSL corpus a hard build
dependency of the art tools, which [F1](#f1) explicitly avoids.

**Note.** `span` defaults to `1.0` in the loader (`loader.rs:297`), so a def that declares none draws
in a 1-tile frame today. The wolf (`size 1.125`, no span) therefore draws 1.125 tiles of sprite into
a 1-tile frame. Whether that currently clips is worth a look, but it is `square-128`'s territory, not
this stream's.

## F3 — `--pad` on an atlas {#f3}
_2026-07-28 · **REVISED at P0.5** — the per-cell guard already exists; use it, don't rebuild it_

**Chosen.** `--pad` **skips** any leaf carrying an `atlas.json`. The per-cell guard on an atlas is
the existing `GRID_INSET_FRAC` path, and `generate_tile.py` is taught to emit its `padU`/`padV`
(which it does not emit at all today — [I8](issues.md#i8)).

**Why the original was wrong.** It proposed teaching `pad_maps.py` to inset every cell. That would
have been a second implementation of a mechanism `bin/art` already ships: `GRID_INSET_FRAC`
(`bin/art:161`) records `padU = f/cols`, `padV = f/rows` into `atlas.json`, the server folds it into
the manifest, and `TextureResolver.ts:202` narrows each cell's UV rect by it. The existing comment is
explicit that this path is *sampling* inset and that **"ADDING padding — growing the atlas with
gutters — is a separate slice/resize/pack tool, not this inset path."**

**And the two are not equivalent — the existing one is better here.** `--pad` shrinks content and
replicates edges, which on an atlas resamples every cell and pulls the sheet off its grid (1024 with
a 1 px canvas ring is 1022, not 8 × 128). The inset changes no pixels at all: it spends a margin of
already-authored art as the guard. For a full-bleed ground sheet that margin is free.

**What stays true from the original.** `--pad` as it currently behaves on an atlas is a live bug
([I2](issues.md#i2)) — guarding the canvas edge only, leaving every interior boundary unguarded. The
fix is to skip, not to extend.

**Rejected — leave `--pad` applying to atlases.** It is actively wrong there, per I2.

**Rejected — inset in `pad_maps.py` anyway.** Two mechanisms writing the same `atlas.json` field,
with the shipped one already wired to the client.

## F4 — The old per-cell linked folders {#f4}
_2026-07-28 · **RESOLVED by the user** — convert to the go-forward shape, delete the folders_

**Decided.** `1.l.0.diffuse.png` becomes `sprite.l.0.png` (the same kind/leaf-level source every
other kind uses) and the per-cell `1..16/` folders are deleted. This lands the design's decision 7
(a linked form is ONE held-whole atlas, no per-cell variant folders) rather than preserving the
shape it supersedes.

Applies to all five linked kinds, not just blueprint: `blueprint/wall`, `brick/wall`,
`default/rock`, `flecked/rock`, `smooth/wall` all carry the same `1.l.0.diffuse.png` source.

## F6 — Sprite is an ARCHIVE; remaster normalises every map to the target square {#f6}
_2026-07-28 · **decided by the user**, and it dissolves B1(b)_

**Decided.** `sprite.*.png` is held at whatever resolution it was authored at and is **not a
deliverable**. On remaster, every derived map is scaled to the span-derived square — in **all** cases
where the resolution does not already match, up or down — with a console **warning on upscale** so
undersized sources are visible as a worklist.

**Why this is better than what I proposed.** I framed B1(b) as a choice between upscaling
`smooth/wall` (fake resolution) and re-authoring it (content work). Both were wrong framings: the
master stays at its authored resolution either way, so nothing is lost and the decision is not
irreversible. Re-authoring later re-runs the same normalisation and the deliverable improves with no
pipeline change. And the art is coarse, stylised and flat-shaded, which upscales far better than
photographic detail would — plus there is room to sharpen on the way up later.

**What it changes here.** The target square must be enforced on the paths that currently pass a
sheet through verbatim (`_split_variant_leaf`, and the held-whole branch of `_split_variant_sheet`),
not only on the blob paths where `EMIT_SIDE` already does it.

`textures/biome-tile/default/blueprint/wall/1..16/` is the superseded per-cell split; the
[design](../../components/dev/textures/design/texture-layout/README.md) marks held-whole atlases ✅
and the old shape ⚠️ NOT YET. Whether this stream folds them or merely stops adding to them is a P4
call once the linked 4×4 grid is actually being emitted. Recorded now so the choice is deliberate
rather than accidental.
