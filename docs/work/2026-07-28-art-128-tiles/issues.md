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

## I4 — `thing.size` and `thing.span` are different numbers {#i4}
_2026-07-28 · read at plan time from `content/visual/things.rd`_

Both are "in tiles" and they are **not** the same field:

- `thing.size` — the sprite's draw scale in tiles. Conifer `2`, wolf `1.125`. Fractional.
- `thing.span` — the frame's pow2 tile span, i.e. what the texture square derives from. Conifer `2`.

The wolf makes the distinction concrete: `size 1.125` is not pow2 and cannot be a span, so its frame
must span 2 tiles while it draws at 1.125. **Size in tiles ≠ the square it packs into**, and any code
here must take `span` (or the footprint), never `size`.
