# SQUARE back to 128 — split the art dial from the lighting dial — 2026-07-28

_Component: [`client/webgl`](../../components/client/webgl/) · `client/webgl/src/game/viewport/squareMath.ts`
is the one file that decides this. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
findings in [`issues.md`](issues.md). Authority: [`VARIABLES.md`](../../VARIABLES.md) — the fixed slot
grid and the texel families are ratified there and this stream conforms code to it._

## What the user asked for

**`SQUARE` = 128, lighting stays at 64 texels/tile, shadow stays at 16 texels/tile.**

Shadow needs no work: `TEXTILE_UNIT = SQUARE / UNIT` and `UNIT = SQUARE / 16`, so it is 16 by
construction at any `SQUARE`. Units-per-tile is a WORLD invariant — the wire and the data records store
tile+unit positions — so the shadow map stays 512×256 whatever happens here. That leaves two real
moves: raise `SQUARE`, and stop the lightmap following it.

## Why this is a split, not a revert

`2b1025a` ("SQUARE 128 -> 64 — lighting 3.3× cheaper, 192 MiB freed, art softer") bought a real win,
and its own comment says exactly where the win came from:

> this is the single dial on the FINE lightmap's size, because `TEXTILE_SQUARE = SQUARE` and the
> lightmap is `SLOTS · TEXTILE_SQUARE`

That identity is the problem. **One constant was steering two unrelated things** — how sharp the art is,
and how many texels the lighting pass shades — so buying the lighting win *required* paying with art
resolution. There was no way to have one without the other.

Split them and the trade disappears:

| | at `SQUARE` 64 (now) | after this stream | |
|---|---|---|---|
| art maps (albedo, normal, surface, zdepth) | 2176 × 1152 | **4352 × 2304** | 4× — the cost we choose to pay |
| lightmap | 2048 × 1024 | **2048 × 1024** | unchanged — the A/B's win is KEPT |
| shadow | 512 × 256 | 512 × 256 | invariant |
| presence / buckets / dirty | 32 × 16 | 32 × 16 | invariant |

So the stream gives back the ~120 MiB of art-map VRAM the A/B freed, and keeps the ~48 MiB and the
**3.3× lighting** it freed. Those figures are derived from texel counts, not measured — [P0](todo.md)
measures them before anything is changed, because the whole premise is that the two costs separate
cleanly and that is a claim, not a fact.

## The motivating observation

The wolf reads soft and mushy next to the conifers (user screenshot, 2026-07-28). At `SQUARE = 64` a
1.125-tile wolf is drawn into **72 px** and every master is capped to 64 px per tile on the way in;
at 128 the same wolf gets **144 px**. That is the hypothesis, and it is checkable — but it is not the
only candidate (the sprite may simply be authored soft, or lose detail in the channel-pack), so
[P0](todo.md) pins the cause down before [P3](todo.md) re-masters anything.

## Design stance

- **`VARIABLES.md` wins outright.** It already specifies `SQUARE = 128`, `REFERENCE = 3584 × 1536`, the
  lod ladder `128 → 32`, and `ppu = SQUARE/(16·2^lod)` = 8 at lod 0. The code is what drifted. The one
  row this stream genuinely CHANGES is the lightmap's, which currently claims 128 / 4096×2048
  ([I1](issues.md)).
- **Split first at unchanged values, then raise** ([F3](forks.md#f3)). Introducing `TEXTILE_LIGHT` while
  it still equals 64 must be a provable no-op; only then does `SQUARE` move. Flipping both at once
  leaves an identity failure with two suspects.
- **`FINE_RATIO` must come out unchanged at 4.** It is `lightmap texels / shadow texels` = 64/16 today
  and 64/16 after. The fine-refine loop in `LIGHT_FRAG` is therefore untouched, and if that number moves
  the split was done wrong.
- **The `uLSlot` hazard is the known trap.** [`Viewport.ts:453`](../../../client/webgl/src/game/viewport/Viewport.ts)
  feeds the lightmap `win.slotPx` (= `SQUARE >> lod`) and its comment records that hardcoding
  `TEXTILE_SQUARE` there "was right only at lod 0 and sampled ~2^lod off everywhere else". Decoupling
  flips which of the two is correct. This is precisely the failure class
  [`2026-07-24-map-compatibility`](../2026-07-24-map-compatibility/README.md) exists to police, so it
  gets its own item and its own zoom-sweep acceptance.

## Done when

`SQUARE = 128` with the art visibly sharper at area1, the lightmap still 2048×1024, `FINE_RATIO` still 4,
corridor↔brute identity 0 differing texels, a clean zoom sweep 1 → 0.25 → 1 with no lod misregistration,
and the lighting pass ms within noise of the `SQUARE = 64` baseline. `VARIABLES.md` and the code agree
on every constant named here.
