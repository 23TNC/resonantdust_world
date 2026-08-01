# Completed — lighting + shader rework

_The verification log: dated entries saying what landed and **how it was checked**. Append-only._

_Nothing yet — the stream is planned, not started._

## 2026-07-31 · P0 — ground rules, formats, and the instrument

### The gate is met

`shadowGather.ts` and `coldShadowData.ts` are gone, the strip is `done` 20/20, the page loads unlit at
1 draw. This stream builds from that floor.

### `RGB10_A2` proven on the metal, not assumed

Added as a `TexFormat` (`gl.RGB10_A2` / `UNSIGNED_INT_2_10_10_10_REV`) plus a boot assert in
`renderer.ts`, in the style the file already uses for the float extensions — and for the reason it
gives: **this class of failure is silent.** An unrenderable attachment makes the FBO incomplete, and a
lighting pass drawing into an incomplete FBO writes nothing while raising no error, which on screen is
indistinguishable from "the lights are off".

`assertRenderable()` allocates a 1×1 texture, attaches it, and asks the driver rather than trusting
the spec's renderable-format table.

Then verified the part the assert *cannot* check — that it actually blends:

| | |
|---|---|
| FBO complete | **true** |
| two `ONE,ONE` draws of 0.25, read back | **128/255 = 0.502** |
| `getError()` | 0 |

An integer format would have read back 0.25 (ES 3.0 §15.1.4 skips blending for integer formats — the
trap the existing comment documents). This is fixed-point, so it accumulates, and
[F2](forks.md#f2)'s premise holds on this machine.

### The instrument: `__framecost()`

[`frameCost.ts`](../../../client/webgl/src/game/viewport/frameCost.ts) — hand-driven ticks, wall clock
with `gl.finish()` at both ends, median of 5, plus an exact draw count. Its header records **why not
timer queries**, with the measured numbers from [strip I7](../2026-07-31-lighting-strip/issues.md#i7):
every faster drain returns plausible partial data rather than erroring.

Reproduces the floor across two consecutive runs — **0.035 and 0.028 ms median, 1 draw**, each inside
the other's spread.

> **The plan's acceptance said 0.045 ms and that number was stale** — it is P1's figure, and P2/P4
> took the floor to 0.028 by deleting `moverDirty`'s per-frame record rewrites and the cursor light.
> Recorded rather than quietly re-baselined; the instrument agreeing with the *current* floor is the
> check that matters.

### `reachFromIntensity` — one definition, two languages, one file

[`lightReach.ts`](../../../client/webgl/src/game/viewport/lightReach.ts) holds the TS function **and**
the GLSL, deliberately in the same file: [F6](forks.md#f6)'s binding constraint is that the CPU
building each tile's light set and the GPU bounding the walk cannot disagree, and separate files is
how that starts.

Falloff `L(d) = I / (1 + (d/d0)²)` solved at `L = ε`, with `ε = 1/255` (under one 8-bit level) and
`d0 = 1 tile`:

```
d = d0 · sqrt(I/ε − 1)
```

**Full intensity lands on exactly 16 tiles** — chosen so the worst case content can author is the same
worst case the stream's headline prices.

`__reachcheck()` runs the GLSL against the TS at **all 1024** `u10` values:

| | |
|---|---|
| worst absolute difference | **2.3 × 10⁻⁵ units** (at intensity 575) |
| agree (< 1e-3) | **true** |
| max reach | **16 tiles** |

That residual is float32 rounding between the two evaluations, not a difference in logic.

Also added `UNITS_PER_TILE = 16` to `squareMath.ts` — a world invariant that was a bare literal in a
dozen derivations.

**Verified:** page loads clean at the fixture, `getError()` 0, frame still 1 draw at 0.030 ms.

## 2026-07-31 · P1 — the record layer

[`records.ts`](../../../client/webgl/src/game/viewport/records.ts) — `prim_data` + `definition_data`,
their mirrors, the allocators, and the lane assertions. Two separate `RGBA32UI` textures (1 MiB +
256 KiB) rather than banded regions of one: with no `set` nibble there is nothing to gain from packing
them together, and separate textures mean **an index is just an index** — which is the whole point of
the flat model.

### Every acceptance, checked against the live scene

`__records()` runs on the real renderer, not a fixture:

| acceptance | result |
|---|---|
| allocation never returns the sentinel ([F8](forks.md#f8)) | **true** over 64 allocations |
| `base + rotation`, 4-rotation billboard | frames `[10, 11, 12, 13]` — one add, no special case |
| `base + rotation`, 16-cell linked tile | rotations 0/7/15 → `[200, 207, 215]` |
| `frame.span` un-biases | **true** (stored 3, reads 4) |
| rotation clamp ([F7](forks.md#f7)) | asked 9, allocated 4, **got 3** |
| lane too wide throws, not truncates | intensity > `u10` ✓, unit > `u16` ✓, span 0 ✓, valid write succeeds ✓ |
| round-trip a real prim | **exact** — 1764/1032 units, rotation 2, intensity 512 |

**One expectation was mine and wrong.** The first run reported `rotationOver4Bits: false` — writing
rotation 99 did not throw. That is correct *by design*: `writePrim` clamps through `clampRotation`
before the lane check, which is what [F7](forks.md#f7) specifies (validate on write, so the GPU pays
nothing). The test asserted a throw where the contract promises a clamp. Corrected to assert the
clamp — the code was right and the check was measuring the wrong contract.

### The atlas packer already existed ([I8](issues.md#i8))

Verified by sampling rather than rebuilt: four equal quadrants of a 256×256 frame,
`fullframe.span == 2 × frame.span`. **And it disagrees with the design on BL/BR** — live is
`surface [0,1]`, `layers [1,1]`; the doc has them swapped, on the four lines prefixed *"I believe that
puts…"*. The mapping is live, so the doc is what needs the correction. Raised, not silently edited.

### `__buildrecords()` ([I9](issues.md#i9))

P1's last item asked the G-buffer bake to draw from records; the design keeps the bake (art → channels)
and the records (what shaders read) apart, so that was logged as a plan error and its *intent* built
instead: mint a record for every live prim from the resolver, then cross-check each decoded
`frame.x/y` back against the resolver's own frame.

**455 prims, 2 definitions, 0 frame mismatches.** A re-route would prove the records are sufficient to
draw from; this proves they are *accurate*, which is what a shader holding a bare `u16` actually needs.

**Verified:** page loads clean, `getError()` 0, frame still **1 draw at 0.032 ms** — the record layer
costs the render path nothing, because nothing renders from it yet.

## 2026-07-31 · P2 — the per-tile records: `presence` and `light`

Both are one px per tile, 8 × `u16` slots, on a 256×256 toroidal fold (one region — the window is far
under a region, so two tiles sharing a residue can never be on screen together). 1 MiB each.

Built from the **same walk** that mints the prim records, so the record set and the drawn set cannot
end up describing different scenes.

### `light` — the reach relation, exact at the boundary

`buildLights` registers each light into every tile inside `reachTilesFromIntensity`, circular rather
than a square box, with nearest-N eviction. Checked against a full-intensity light at tile (100, 50):

| tile | registered |
|---|---|
| centre (100, 50) | **yes** |
| +16 tiles | **yes** |
| +17 tiles | **no** |

`reachTilesFromIntensity(1023)` is 16, so the registered set ends exactly where the derived reach says
it should. That is [F6](forks.md#f6)'s CPU/GPU agreement confirmed on real data rather than on the
1024-value unit check alone — and it is the direction that matters: a light registering *short* of the
walk's bound is the silent failure (a shadow that never casts at all).

### `presence` — and the assertion that nearly passed for the wrong reason

First run reported `outOfOrder: 0` — but also **`multiReceiverTilesChecked: 0`**. The live scene puts
455 prims on 455 distinct tiles, so there was never a tile with two receivers and **the sort assertion
had nothing to sort**. A green check that tested nothing.

Forced the case instead: 12 receivers onto one tile in **deliberately reversed** layer order, and 10
lights onto another against an 8-slot cap.

| | |
|---|---|
| receivers offered / kept | 12 / **7** (slot 0 is the tile) |
| kept layers | `[5, 6, 7, 8, 9, 10, 11]` — **ascending** |
| kept the topmost | **true** (layer 11 survived) |
| `droppedReceivers` delta | **5** |
| lights: slots used / `droppedLights` delta | **8** / **298** |

Both caps evict, both count, and the eviction keeps the **topmost** receiver — which is the half that
matters, because the per-pixel pass walks `presence[7..1]` and takes the first hit. Dropping the top
of the stack would silently hide whatever the player is actually looking at.

`droppedLights: 298` is large because a 10-light over-subscription at one tile spills across every
tile in all ten reach circles, not just the target — worth knowing the counter is per *slot eviction*,
not per light.

**Verified:** page loads clean, `getError()` 0, frame unchanged at 1 draw / 0.030 ms. The record layer
still costs the render path nothing, because nothing renders from it yet.

## 2026-07-31 · P3 (1/5) — the ¼-scale slot encoding

`LIGHT_SLOTS = 8`, `LIGHT_SCALE = 4`, and `encodeLightChannel` / `decodeLightChannel` in
[`records.ts`](../../../client/webgl/src/game/viewport/records.ts).

Tested through the **real texture path** — a shader writing into an `RGB10_A2` attachment — rather
than through the TS arithmetic, because the arithmetic was never the risk:

| wrote | stored (0..1) | read back |
|---|---|---|
| **4.0** | **1.000** | **4.000** |
| 2.0 | 0.498 | 1.992 |
| 1.0 | 0.251 | 1.004 |
| 0.25 | 0.063 | 0.251 |

**The 4× ceiling holds exactly**, which is the acceptance: the blit this replaces clamped at
`vec3(4.0)`, so overbright is shipped behaviour and `RGBA8` would have clipped each light at 1.0
*before* the sum, flattening falloff near bright sources ([F2](forks.md#f2)).

The small drift on the middle rows is the **readback**, not the storage: `readPixels` was taken as
`RGBA8`, so those numbers are quantised to 8 bits on the way out while the texture holds 10.

## 2026-07-31 · P3 (2/5) — all 8 lights in ONE draw

[`lightPass.ts`](../../../client/webgl/src/game/viewport/lightPass.ts). The slot map is
`LIGHT_SLOTS`× wider than the lighting grid (16384 × 1024), so a fragment derives its light from
`x % 8` and touches exactly one slot — **one draw covers all 8 lights, and no MRT**
([F3](forks.md#f3); MRT hung Chrome twice and is banned stream-wide).

| | |
|---|---|
| draws per lighting update | **2** — one slot pass for all 8 lights, one sum |
| slot map | 16384 × 1024 `RGB10_A2`, 8 slots/texel |
| `getError()` | 0 |

### The falloff is exact, and it is the SAME function reach inverts

`reachFromIntensity`'s GLSL is injected into the shader, so the attenuation and the reach bound cannot
be two different curves — the shader evaluates `L(d) = I / (1 + (d/d0)²)` and the CPU's tile
registration solves the same expression for `L = ε`.

| sample | measured | formula |
|---|---|---|
| the light's own tile | `[1.001, 0.954, 0.883]` | `I/(1+0) = 1.0`, tinted by `color.4` |
| 8 tiles away (128 units) | `0.0156` | `1/(1 + (128/16)²) = 0.0154` |

Agreement to the 10-bit quantum, through the ¼-scale store and the ×4 undo in the sum.

### F9's guard fired on the first run, correctly

The first run read **zero everywhere**. Not a bug: the synthetic light record pointed at prim index 1,
which is a *tree* — `emit_type == 0`. The slot shader re-checks the type lane before trusting a slot
([F9](forks.md#f9)), so it contributed nothing, which is exactly the designed behaviour for a recycled
or mis-pointed index.

Worth recording because it is the failure this project keeps hitting **inverted**: a stale index that
silently produced a *plausible* light would have been invisible. Here it produced nothing, loudly, on
the first look. Fixed by minting a real emitter (`emit_type = 1`, `intensity = 1023`) and registering
that — the guard was right and the test data was wrong.

## 2026-07-31 · P3 (3/5) — cost per light, and the instrument that was lying

**The headline: one lighting update costs ~0.30 ms** at 16 777 216 fragments (8 slots × 2048 × 1024),
which is ~55 Gfragment/s — the first defensible bound on the pass the plan flagged as unbounded.

Getting there meant discovering that **the instrument was wrong** ([I10](issues.md#i10)). The first
result was 0.003 ms flat across N = 1/4/8/16 — flat is suspicious and 0.003 ms is impossible, so it
got chased rather than published:

1. `performance.now()` is **coarsened to 0.1 ms** in a backgrounded tab, and the runs totalled 0.09 ms
2. **`gl.finish()` does not sync** — Chrome's GL lives behind a cross-process command buffer
3. `readPixels` with a **mismatched format** raises `INVALID_OPERATION` and does not sync either

Only a format-matched `readPixels` forces a real sync. `frameCost.ts` now uses one, and its header
carries the whole finding.

**Cost per light is flat in N.** That is expected and worth stating: the slot pass rasterises all
8 slots every time regardless of how many are occupied, so N changes what each fragment *finds*, not
how many fragments run. Making cost track N is what [F1](forks.md#f1)'s delta update is for — the
next item — and this measurement is the baseline it has to beat.

**The unlit floor is restated at ~0.11 ms**, not 0.028. Every earlier ms number in both streams is low
by roughly 4×; they are recorded as-taken rather than retro-edited, with I10 naming what is affected.

## 2026-07-31 · P3 (4–5/5) — the delta path, and add/remove proven EXACT

**`afterRemoveDifferingFloats: 0`, three runs.** Add a light through the delta path, remove it, and
the summed map returns **bit-identically** — the property the old accumulator needed `LIGHT_QUANT` to
fake, here structural.

Getting there took four attempts, and each failure taught the design something:

| attempt | worst residual | what it taught |
|---|---|---|
| emit `new − old`, then write the slot | **0.88** | the scissored slot write used `gl_FragCoord.x` raw, so the **block offset was being read as the texel coordinate** — the delta was right, the slot landed in the wrong place |
| same, offset stripped | **0.0022** | the add deposits a freshly-computed float while the slot *stores* it quantised, so the removal subtracts a different number |
| quantise the prediction in software | **0.0039** | software rounding does not match the hardware's in the last bit. **Predicting what the GPU will store is not a strategy** |
| **withdraw → write → deposit**, reading the slot both times | **5.96e-8** | one FP32 ULP: `a + (−x) + x` re-rounds |
| **+ deposit integer LEVELS, not floats** | **0** | integers under 2²⁴ are exact in FP32, so the pair cancels bit-exactly |

### Two design corrections this forced

**1. Never predict the store; read it.** The update is now three draws — subtract what the slot holds,
rewrite it, add back what it now holds. Every term is read, none is predicted. That is one draw more
than the plan's acceptance ("one slot and one blended draw"), and worth it: exactness is the *next*
item's acceptance, and the two-draw form cannot deliver it. Still one slot — the other seven and the
whole rest of the sum are untouched ([D3](deviations.md)).

**2. [F1](forks.md#f1) was wrong that quantisation had no reason to exist.** It has none for the
**slots** — those are overwritten, never accumulated. But the **sum is still an accumulator**, and an
accumulator that must invert still needs an exact alphabet. It now holds integer quantisation levels
(8 slots × 1023 = 8184, nowhere near 2²⁴), which is exactly the old `LIGHT_QUANT` reasoning arrived at
from the opposite direction.

The sum is also `RGBA32F` rather than the `RGBA16F` F1 assumed: 13 mantissa bits are needed and FP16
has 11. 32 MiB against 8 ([D2](deviations.md)).

### Also

The **blocked** slot layout (slot `s` owns `x ∈ [s·W, (s+1)·W)`) replaced the interleaved one — a slot
has to be a *rectangle* to be scissored, and without that "update one light" would have to touch all
eight.

**The GLSL backtick foot-gun caught me again**, in a shader comment (`` `a + (-x) + x` ``). `tsc`
reported it as a TS syntax error two lines later, which is the only reason it was cheap.

## 2026-07-31 · P4 (1, 3/5) — the shadow buffer, and the slot split corrected

`shadowPass.ts`. **3 px per UNIT, ping-ponged, cleared to 0.**

| acceptance | result |
|---|---|
| per UNIT, not per tile | 512 × 256 units → a **1536 × 256** texture (`unitsX × 3`) |
| sized ~6 MB, not ~24 KB | **6 MB** per buffer, **12 MB** with the ping-pong |
| first frame reads zeros | **0 non-zero texels** across the sampled span |
| `getError()` | 0 |

Cleared to 0 specifically because **0 is the "no caster" sentinel**: an uninitialised buffer would
name real prims, and the adjacency step would trust them. Zero means "take the slow path", which is
the safe direction for garbage to fall.

### The `l >= 4` correction, demonstrated rather than asserted

`pairSlot()` is the one place the split lives:

| lights | px | slots |
|---|---|---|
| 0–3 | 1 | 0, 2, 4, 6 |
| 4–7 | 2 | 0, 2, 4, 6 |

All eight distinct, none past slot 6 (the last valid start for a pair in an 8-slot px).

The check also **runs the design's own `l > 4`** and reports what it produces:
`light 4 -> slot 8` — one past the end of px 1, while px 2's slots 0 and 1 are never used at all.
So light 4 would write outside its px and never shadow ([I1](issues.md#i1)). That is the failure that
reads as "the shadows look wrong" rather than as an out-of-range write, which is why it is worth a
test that names it rather than a comment that mentions it.

## 2026-07-31 · P4 complete — the gather

One draw, three tiers, verified against an exhaustive search.

| acceptance | result |
|---|---|
| the walk runs only when the cheap paths miss | **incumbent 66.9 %**, adjacency 0 %, **corridor 33.1 %** of answered pairs |
| walk vs exhaustive search | **0 differing** occlusion decisions across 131 072 slots |
| the caster type lane is checked at use ([F9](forks.md#f9)) | clearing `cast_type` on all 455 casters takes shadows **3855 → 0** |
| draws per gather | **1** |

### Three bugs found by testing, not by reading

**1. A point-march skips tiles.** The first corridor sampled the segment at intervals, which clips
diagonal tiles — and every skipped tile is a caster that silently never shadows. Replaced with a
**supercover DDA** (Amanatides–Woo) that steps boundary to boundary and visits every tile the segment
touches by construction. This is exactly why the old system used one.

**2. A card is wider than its tile.** Even with the DDA, the walk disagreed with brute on ~1100 slots:
the walk visits the tiles the *segment* crosses, but a caster occludes when its **card** crosses the
ray, and a card overhangs the tile it registers in. Added **x-dilation** — only x, because the card is
horizontal, so a ray crosses each row once. That is the same lesson the old system encoded as its walk
dilation.

**3. My own test counted the wrong thing, twice.** First it compared caster *identity*
([D4](deviations.md)); then it counted raw non-zero words when `px 1/2` pack `(caster, receiver)`, so
the receiver half kept them non-zero and the guard looked broken at 498. Counting caster fields
specifically gives 0.

Worth naming: **two of the three "failures" were the test, not the code** — and the one time I could
have declared success early (`0 differing` on a run where the records had not been rebuilt) the tier
counts were all zero, which is what caught it. A pass with nothing in it is not a pass.

## 2026-07-31 · P5 — the refine, and the first lit frame

[`after/lit-02.jpg`](after/lit-02.jpg) — a point light with radial falloff, and **shadows radiating
from every conifer that occludes it**, cast from stored caster identities and refined at 64/tile. The
whole chain runs: records → presence/light → gather → slots → sum → one blit fetch.

### Where the refine runs ([F12](forks.md#f12))

The design's second pass is per screen pixel, which would make [F1](forks.md#f1)'s summed map
pointless — if lighting is recomputed per pixel there is nothing left to save. So the refine runs
**inside the slot pass at 64/tile**, against a gather that resolved at 16/tile: **4× finer per axis**,
for one `occludes` call and no search, because the identity is already stored. That is precisely the
win the old stream paid 9.29 ms of a 10.88 ms pass for and then deleted.

The price, stated: at zoom 1 a lighting texel is 2×2 screen pixels, so the edge quantises to 2 px
rather than 1. If that ever matters, the fix is to raise `TEXTILE_LIGHT` — the same dial — not to move
the refine and lose the one-fetch display.

### The gate ([F5](forks.md#f5))

Only a texel whose **unit holds a caster for that light** does any refine work: one shadow-buffer
fetch, then one `occludes`. Interior and fully-lit texels do neither. `uDebugGate` emits the gate's
own selectivity so it is a measured percentage rather than an assumption.

### An addressing bug the first lit frame caught immediately

The first render came out at ambient with faint diagonal bands — shadows present, light nearly absent.
The slot pass writes **linearly** from the window origin (`texel x → winCol + x/64`) while the blit
read **toroidally** (`pmod(tile, cols)`). Those agree only when `winCol` is a multiple of `cols`; at
`winCol = 84, cols = 32` they do not, so the light was deposited in one slot and read from another.

Worth recording as a class, not an incident: **a writer and a reader must share one addressing rule,**
and this project has now been bitten by that same shape three times (`uLSlot` vs `win.slotPx`,
`resolvedTilePos`'s reference point, and now this). The shadow bands were the tell — they were in the
right place because the gather and the shadow read *do* share a rule.

### The refine's own cost: **negative**

Measured by differencing the slot pass with the refine on and off — 200 iterations, median of 3, a
`readPixels` sync at both ends ([I10](issues.md#i10)):

| | ms per slot pass |
|---|---|
| refine **off** | 0.4155 |
| refine **on** | **0.3295** |
| difference | **−0.086 ms (−20.7 %)** |

**Turning shadows on made the pass faster**, and the explanation is structural rather than a fluke: an
occluded texel `return`s immediately with zero, skipping the falloff, the tint and the clamp. The
shadowed fraction of the map therefore does *less* work than it did unshadowed, and that saving
exceeds the gate's one buffer fetch plus one `occludes` call.

The gate ([F5](forks.md#f5)) is what makes this hold — an ungated refine would test every texel,
including the ~97 % with no caster, and the sign would flip. Recorded with the caveat that it is
scene-dependent: a scene with no shadows at all would show the gate's cost with none of the early-out
saving, and the number would be small but positive.

## 2026-07-31 · P6 — errors, limits, and what a failure looks like

### The index-0 audit ([F8](forks.md#f8))

Every record fetch in both shaders, with the guard that precedes it:

| site | index-0 guard | type-lane check |
|---|---|---|
| `lightPass` `refineOccluded` | `if (c == 0u) return false` | `cast_type != 0` |
| `lightPass` slot pass | `if (idx == 0u) { … return; }` | `emit_type != 0` |
| `lightPass` single-slot write | `if (uPrimIndex == 0) { … return; }` | `emit_type != 0` |
| `shadowPass` `occludes` | `if (c == NONE) return false` | `cast_type != 0` |
| `shadowPass` light loop | `if (li == NONE) continue` | `emit_type != 0` |

**Five sites, five guards, five type checks.** Both `fetchDef` calls sit downstream of an already-
guarded `fetchPrim`, so their block index comes from a validated record; block 0 is never allocated
(`defNext` starts at 1) and reads as zeros, which decode to a 1-unit card — inert rather than wild.

### NaN guard

`clamp()` is **undefined on NaN**, so a single bad record could put a NaN in a slot — and the delta
path would then blend it into the summed map, where it poisons every subsequent add and **cannot be
withdrawn**, because the withdrawal reads the same NaN. Guarded by comparing the value against itself
before the clamp.

### The failure-mode table — what a user SEES when a limit trips

| symptom on screen | cause | where to look |
|---|---|---|
| one light missing in **one tile**, fine elsewhere | >8 lights reach that tile; nearest-8 evicted the rest | `droppedLights` non-zero |
| a **billboard is lit but casts nothing** | its `cast_type` is 0, or its definition never allocated a rotation | `debugPrim(id).castType` |
| a stacked object **never receives**, the ones under it do | >7 receivers on the tile; the cap keeps the **topmost** | `droppedReceivers` non-zero |
| a light **stops exactly at a tile boundary** | its intensity puts `reachTilesFromIntensity` right on that ring — a cap, not a bug | `__reachcheck()` |
| the world is lit but **flat, no shadows** | the gather never ran, or `uRefine` is 0 | `__gather()` tier counts all zero |
| shadows in the **wrong place**, light in the right place | a writer/reader addressing mismatch — this has bitten three times | compare the writer's rule to the reader's |
| everything **black** | `uLit` on with an empty lightmap; or a light record with `emit_type` 0 | `__lightpass()` sum at the light tile |

The last two are the ones that cost time, because both render *plausibly*. Neither raises a GL error.
