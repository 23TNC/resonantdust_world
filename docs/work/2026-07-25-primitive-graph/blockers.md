# Primitive graph — blockers

_Things needing human input. Open → resolved (archive resolved with a date). Newest first below the
resolved set. **No open blockers** — B-1…B-5 all resolved. — neither stops P10's remaining code, but both are visible
in the product right now._

## B-4 — the EXISTING world has no torches — RESOLVED 2026-07-26, no decision needed
Dissolved by [torch-thing](../2026-07-26-torch-thing/README.md): torches are now seeded at world init into the
zone the camera opens on, so the spawn area is lit regardless of when its terrain was generated. And the
already-seeded zones regenerated as a **side effect of `redeploy --force`** (a module republish resets the cold
shards), so the destructive option this blocker was asking about never had to be taken. Verified: exactly 3
torches world-wide, at exactly the seeded cells, spawn lit.

<details><summary>B-4 as originally raised</summary>
**What.** Worldgen output is **persisted**. The torch scatter ([P11](todo.md)) only runs when a zone is
first seeded, so it applies to zones generated *after* the rule landed. Measured: the spawn area
(`focus=100,50`) has **0 torches across 3,429 prims**, while a never-visited region (`focus=900,700`) has
**74**. The edge hot-reloads the corpus per zone seed, so nothing is stale — those zones were simply already
written.

**Why it needs you.** The world near spawn is where you actually look, and it is dark. Fixing it means
discarding stored zone data, which is your data and not mine to drop:
1. **Leave it.** New territory is lit, old territory is not. Zero risk, but the default view stays dark and
   every future lighting change is awkward to eyeball near spawn.
2. **Wipe the seeded zones** so they regenerate with torches. Worldgen is deterministic and `seed` is
   idempotent, so terrain comes back identical — but anything a player has since CHANGED in those zones is
   lost, and I do not know whether that matters here.
3. **Move the default focus** to virgin territory. No data touched; the lit world becomes the one you land
   in. Cheapest, and reversible by editing one URL default.

**Suggested path:** (3) for now — it is free and non-destructive. Take (2) only if you are confident nothing
in the seeded zones is worth keeping, and say so explicitly; I will not wipe stored world data on my own
initiative.

</details>

## B-5 — over-bright light — DISSOLVED 2026-07-26 by the user: clamp PER LIGHT, pre-write
Not a decision after all. The user's chain, which is correct and which I had muddled:

1. The lightmap is `RGBA32F`; FP32 is exact for integers below **2^24**.
2. **2^24 > 255 × 65,535.**
3. So if each of the at-most-65,535 lights (the u16 id space) deposits at most 255 per channel, every
   possible sum is lossless.
4. The ACCUMULATOR cannot be clamped — clipping A+B at 255 makes subtracting B leave 0 where 255 is right,
   so the light becomes un-removable.
5. But each LIGHT can be clamped, pre-write. A raw 512 simply clamps to 255 before it joins the sum.
6. Since the id space caps the count anyway, **no texel can ever exceed 2^24. By construction.**

**My error, corrected.** I claimed a per-light clamp meant "no brighter braziers". False: clamping caps a
light's PEAK, not its profile. At `intensity 4` every distance is still 4×, so the core saturates over a
WIDER radius and the falloff stays brighter further out — which is what a brighter light looks like. So
intensity above 1 is free, and there is no tradeoff to weigh.

**Implemented:** `acc += min(col * contrib, vec3(1.0))` in `LIGHT_FRAG`. Verified a no-op at `intensity 1`
(peak unchanged at 247), so the guarantee is now enforced rather than aspirational, with no visual change.

<details><summary>B-5 as originally raised</summary>
**What.** With the additive accumulator ([P10](todo.md)) lights sum without a per-light clamp, so overlapping
torches genuinely exceed full brightness — measured 308–321 where 255 is one light at full. Previously the
`rgba8unorm` map clamped every texel at 1.0 and the question could not arise.

I put a **placeholder** in the blit: `light = ambient + min(irr, 4.0)`. That 4.0 is arbitrary — I picked it
to stop a runaway, not because it looks right.

**Why it needs you.** This is a LOOK decision, not a correctness one, and it is the first thing a player
sees. The options read very differently:
1. **Hard clamp** (current). Overlapping lights flatten to a uniform bright patch — detail disappears exactly
   where the most light is.
2. **Reinhard / filmic tonemap** — `irr / (1 + irr)` or similar. Overlap keeps rolling off instead of
   clipping, so bright areas retain shape. Standard, cheap, and changes the whole image's feel.
3. **Exposure + clamp** — scale by a tunable exposure before clamping, so you set where "full brightness"
   sits rather than inheriting it from `LIGHT_QUANT`.

**Suggested path:** (2), because the accumulator's whole point is that many lights overlap and a hard clamp
throws that information away at precisely the interesting texels. But the curve is yours — tell me the look
and I will fit it.

</details>

_No open blockers._

---

## Resolved

## B1 — Four layout semantics ✅ RESOLVED by the user (2026-07-25)
1. **Child offset signedness** → **bias-8** per nibble (−8..+7).
2. **CPU-resolved vs GPU-walked** → **both possible** via `u16 parent_id` on leaves + a `child` bit on
   `prim_data`; resolved further in B2 ([F1](forks.md#f1)).
3. **`rotation`/`layer` precedence** → dissolved: the shader uses the **definition's** rotation; stored
   rotation is a CPU **reconciliation signal** driving definition swaps ([I4](issues.md#i4)).
4. **Header count width** → dissolved by **fixed 8-px commands** ([I5](issues.md#i5)).

## B2 — The revision's two items ✅ RESOLVED by the user (2026-07-25)
1. **`emitter_radius` restored** — `light_data ALPHA: u12 reach | u8 radius | …` ([I10](issues.md#i10)).
2. **Resolved position: the CPU stamps it.** The leaf spends its whole RED lane on
   `u16 parent_id | u8 resolved_tile | u8 resolved_unit`; resolution walks root → child offsets → leaf.
   The authored offsets stay in GREEN. The GPU never walks the graph in the hot loop
   ([I11](issues.md#i11)). Also settled: **`id = 0` is the global sentinel**, so partial commands pad
   with zeros ([I7](issues.md#i7)), and `inherit_rotation` moves to `definition_data` + `prim_data` with
   **one-step** inheritance.

## B3 — `resolved_zone` alongside resolved tile/unit ✅ RESOLVED by the user (2026-07-25)
**Resolved.** The user supplied the deciding reasoning: `light_presence` is a **reach** relation (a light
knows exactly which tiles it affects, so it registers itself on all of them) while `billboard_presence`
is a **containment** relation (shadows are combinatorial — every billboard × every light — so a
billboard can only register the tiles it *occupies*, and the gather projects the shadow from there).
That is precisely why a light needs `resolved_zone` and a billboard does not. `u8 resolved_zone` is in
`light_data` ALPHA, landed in VARIABLES at P0, and **in use since P2c** — `resolvedPos()` reconstructs a
light's absolute position from `zone|tile|unit` by nearest-congruent (256-tile period), and
`resolvedTilePos()` is its 16-tile-period sibling for the containment case.

_(Left marked OPEN long after it was settled, which silently released the continuation hook — see
[I21](issues.md#i21). Close a blocker in the commit that closes it.)_

<details><summary>original analysis</summary>

`resolved_tile` is `x:4|y:4` = the **in-zone** tile, so reconstructing a light's
absolute position from a fragment has a **16-tile period** — unambiguous only within **8 tiles**. But
`LIGHT_REACH` is **12 tiles** today (and the `u12 reach` field allows far more), so a light 12 tiles north
would reconstruct as 4 tiles south. The gather genuinely needs the absolute position: it computes
`toL = Lxy - P` for falloff/N·L and marches the corridor from the light. This is the same
position-ambiguity class as the two zoom regressions, so I don't want to build on it.

**Proposed fix (already applied in the README, needs your yes):** also stamp **`u8 resolved_zone`**
(in-region zone, `x:4|y:4`). Zone+tile+unit → a **256-tile** period, unambiguous for any reach under 128
tiles; region stays inferred by nearest-congruent, exactly as the region-torus fold already does. It
costs nothing structurally — the room is in reserved space:
- `light_data`  ALPHA: `u12 reach | u8 radius | u8 resolved_zone | u4 reserved`
- `billboard_data` ALPHA: `u8 resolved_zone | u24 reserved`

**Alternative if you'd rather not spend the bits:** cap effective light reach below 8 tiles (currently
12) so the 16-tile period is unambiguous — but that's a gameplay/visual constraint imposed by an
encoding, which seems like the wrong trade.

</details>

## P10's memory gate CLEARED — 2026-07-26 (not a blocker; recorded so the state is visible)
[P10](todo.md) (the additive `RGBA32F` lightmap, [F11b](forks.md#f11b)/[F11b.1](forks.md#f11b1)) was gated
on the lightmap's memory: at the old world-fixed density `RGBA32F` cost **465 MB**, which was not viable.

**[2026-07-26-textile-slot](../2026-07-26-textile-slot/README.md) delivered that gate.** Measured on the
fixed slot grid, the single collapsed accumulator is **128 MiB, constant at every zoom** (4096×2048 ×
16 B on the final 32×16 grid) against the 64 MiB of `RGBA8` tiers it retires. P10 is executable on memory
grounds — and the RGBA32F half has since LANDED.

**Still open before P10 can start** (work, not input): P9 was retired in favour of that stream, and
[P4 there](../2026-07-26-textile-slot/todo.md) — per-light `coarsest_lod` — is the reverse dependency,
since it has no consumer until this accumulator exists. Land the accumulator first, then P4 immediately
after; neither needs a decision from you.
