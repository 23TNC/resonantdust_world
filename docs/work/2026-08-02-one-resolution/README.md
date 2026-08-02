# One resolution — remove the atlas LOD ladder — 2026-08-02

_Component: [`client/webgl`](../../components/client/webgl/). The user's directive: the
old-game theory was "don't ship every texture at full size", but THIS game is heavily
zoom-reliant and always passes the maximum texture size — so the atlas holds complexity
that no longer needs to exist. **Hold ONE resolution of each stem's map set and resize
in the shaders at draw/lighting time; mipmaps + AA (recently restored) do the
minification the ladder used to fake.**_

## The two things called "lod" — one dies, one is renamed

1. **The atlas LOD ladder** (DIES): `TextureResolver`'s per-size machinery —
   `LOD_SIZES`, `targetPx`/`setTargetLod`, per-(stem, size) co-packs and IndexedDB rows,
   the `pickLodForSize` below/above chooser, the `FLOOR_LOD` preview tier, upgrade
   kicks re-emitting `onLoad`, `LodPool`'s per-size duplication, and the def-block swap
   that made "the bound atlas page switches with zoom" a standing gotcha. One stem =
   one co-packed frame at the maximum size, forever.
2. **The slot-grid level** (RENAMED, [F1](forks.md#f1)): zooming re-sizes each slot's
   texels (`slotPx = SQUARE >> level`) so textures never re-allocate — the textile-slot
   design. That concept is load-bearing (the display torus, the lighting chain's
   texels-per-tile, the bakes) and keeps working exactly as it does; it just stops
   sharing a name with the thing being deleted.

## What gets simpler

- The resolver: `packed` collapses `Map<stem, Map<size, frame>>` → `Map<stem, frame>`;
  no target tracking, no tier picking, no preview gate, no upgrade churn.
- The records: ONE def block per stem — the CPU never swaps def ids with zoom; the
  atlas page registry stops moving under the lighting pass when the wheel turns.
- The def SEED lane (atlas px-per-unit × 8) stays — it exists because STEMS differ,
  not because sizes did — but it becomes constant per stem for its lifetime.
- The bakes sample the one master through mipmapped samplers: minification quality is
  the mip chain's job, not a ladder of pre-shrunk files.

## What must NOT regress

The bake samples at `slotPx` texels/tile — at zoom 0.25 that is a 4× minification of
the master, exactly the case the ladder existed for. Acceptance throughout is A/B
captures at the deepest zoom against pre-stream captures: equal or better, or the mip
sampling is wrong. Memory is the other watch: one 128-px co-pack per stem replaces up
to three sizes — the pool should SHRINK; `lodStats`' successor counter proves it.

## The packing law rides along

A concurrent stream apparently broke away from pow2 packing (the pool only WARNS on
violations — which is exactly how it slipped). Since this stream rebuilds the pool
anyway, it also settles and ENFORCES the packing convention first: P0 audits the live
atlas for violators, [F3](forks.md#f3) records the pros/cons and fixes the law
(square pow2 stays — every addressing invariant assumes it; ES 3.0's NPOT support only
buys margin bytes), the warn becomes a reject, and violators are re-ingested before the
one-resolution rebuild lands on top.

## Out of scope

The server keeps deriving + serving lower sizes (`/textures` routes untouched — the
client simply stops asking); pixijs (legacy); restoring zoom 0.125 (a loading-priority
question, unchanged from textile-slot B-1).
