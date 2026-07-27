# Completed — verification log

## 2026-07-27 — FULL RE-PROFILE, clean scene, fixed harness

One light, reach 16, 6-tile orbit through `movePrim`, zoom 0.5, `SQUARE` 64, fresh load. **Cold-pass-only
GPU timer** (each program is drawn twice per frame — cold then hot) and, for the substeps, a **deterministic
workload**: light frozen, `rebakeAll()` every frame, 100 % dirty.

### Top level

| stage | where | ms |
|---|---|---|
| **gather** | GPU | **2.59** (2.42–2.89) |
| **lighting** | GPU | **0.53** (0.52–1.31) |
| **GPU total** | | **3.13** |
| `buildCasters` | CPU | 0.6 |
| `buildPresence` | CPU | 0.2 |
| `buildDirty` / `flush` | CPU | ~0 |

Orbiting (non-deterministic) reads 2.68 / 0.36 / **3.04** — same place, wider spread (1.75–3.39 on the
gather), which is why the substeps below use the frozen workload.

### Gather substeps

| level | includes | ms | substep |
|---|---|---|---|
| 1 | dirty gate + window mapping + `P` | 0.038 | 0.038 |
| 2 | + `receiverAt` | 0.072 | +0.034 |
| 3 | + `allBillboard` corner test | 0.074 | +0.002 |
| 4 | + presence + light loop, no walk | 0.088 | +0.014 |
| **0** | **+ `walkShadow`** | **2.585** | **+2.497 (96.6 %)** |

### Inside `walkShadow`

| level | includes | ms | substep | share |
|---|---|---|---|---|
| 1 | DDA setup | 0.088 | 0.088 | 3 % |
| 2 | + traversal | 0.102 | +0.014 | 1 % |
| 3 | + bucket fetches | 0.279 | +0.177 | 7 % |
| 4 | + slot unpack | 0.398 | +0.119 | 5 % |
| **0** | **+ `casterOne`** | **2.576** | **+2.178** | **85 %** |

Slot unpack fell **0.369 → 0.119 ms** — that is the dense-bucket `break` doing exactly what it should.

### Lighting substeps

| level | includes | ms | substep |
|---|---|---|---|
| 1 | dirty gate + window mapping + `P` | 0.088 | 0.088 |
| 2 | + `receiverAt` | 0.406 | **+0.318** |
| 3 | + normal | 0.428 | +0.022 |
| 4 | + light loop, no shadow fetch | 0.515 | +0.087 |
| **0** | + coarse shadow lookup | **0.535** | +0.020 |

`receiverAt` fell **0.978 → 0.318 ms** (−67 %) from its own `break`, and is now 59 % of a much smaller pass.

### Where the frame stands

| item | ms | share of 3.13 |
|---|---|---|
| **`casterOne`** | **2.18** | **70 %** |
| `receiverAt` (lighting) | 0.32 | 10 % |
| bucket fetches + unpack + DDA | 0.40 | 13 % |
| everything else | 0.23 | 7 % |

**`casterOne` is now 70 % of the GPU frame on its own** — up from 54 % this morning, because everything
around it got cheaper. [F4](forks.md#f4) is the only remaining lever that touches it.

### Session total on this fixture
**11.36 ms → 3.13 ms (3.6×)**, and VRAM **272 → 80 MiB**, from: deleting the shadow-edge refine,
`SQUARE` 128 → 64, and the dense-bucket early outs.

_Dated entries: what landed and **how it was checked**. Append-only; authoritative for what's done and why we
believe it. Items live in [`todo.md`](todo.md) with their boxes ticked — this file records the evidence._

_Standing rule for this stream ([I3](issues.md#i3)): an entry here names the **output** that was read — a
`debugReadShadow(cls)` hash / byte population, or a frame time — not a JS field, an input count, or a
screenshot impression. Three verifications this session were false because they stopped at the near end of the
pipeline._

## 2026-07-26 — I3 root-caused and fixed: a carrier prim's position is now written every frame

**What landed.** `carriedLightFor` wrote `PRIM_BASE + prim`'s position *only* inside its
`prim === undefined` allocation branch. Moved that write out into an `else` that refreshes the **position lane
only** (G and both id lanes carried through from the mirror, so the carried-piece slots
`writeCarriedLight` is about to read are untouched). Also threaded `from` — the position a light is leaving —
out of `writeCarriedLight` and into the caller's `markLightDirty`, closing [I1](issues.md#i1) in the same
change, since the caller cannot recover the old position once the map entry is overwritten.

**How it was checked.** In-browser at `?focus=104,55&zoom=1`, reading the OUTPUT:

- **Before.** All three content torches resolve `def = −1` — they carry no sprite, so `definitionFor` returns
  −1 and `buildCasters` `continue`s before `billboardDataFor`, which is the other (working) writer of a
  carrier position. Orbiting them gave `primMoved: true, recordMoved: false`: the prims moved, the light
  records stayed pinned at their allocation position.
- **After.** `lightRecordsMoved: true`, and `debugReadShadow(0)` hashes across three samples were
  **1561431880 → 3834622774 → 4097208919** (non-zero populations 101 297 / 101 481 / 101 161). The map now
  changes every frame, on a large non-zero population.

That hash triple is the acceptance this stream was built around: the same readback returned a **single
constant** hash with the orbit on and off before the fix.

**Corrects [primitive-graph I40](../2026-07-25-primitive-graph/issues.md).** Its diagnosis — `stepOrbit`
writing a discarded per-frame read model — was wrong. `SquareCache.standingPrims()` pushes **references** to
the stored prims, so those writes always landed. The observation was real; the mechanism was invented, and I
had written it into three documents before checking the four-line method that disproves it.

## 2026-07-26 — I2 confirmed: the zoom cliff is a dirty-FRACTION effect

Measured with 3 orbiting torches (reach 16). Full table + the corrected cost model in [I2](issues.md#i2):

| zoom | lod | map (tiles) | dirty/frame | dirty fraction | ms static | ms moving | fps moving |
|---|---|---|---|---|---|---|---|
| 1.0 | 0 | 512 | 512 | **100.0 %** | 8.33 | **43.06** | 23 |
| 0.5 | 1 | 2 048 | 1 363 | 66.6 % | 8.33 | 28.34 | 35 |
| 0.25 | 2 | 8 192 | 1 811 | 22.1 % | 8.33 | 8.33 | **120** |

**The model that fits:** `work ∝ fraction × (slots × slot_texels)` — a **constant** texel budget of which the
dirty fraction is re-baked. Taking zoom 1 as reference, 43.06 × 0.666 = 28.7 predicted vs **28.34 measured
(1.3 %)**. My pre-measurement model (constant per-tile cost, varying tile count) was wrong in both halves,
which cancelled and left the conclusion standing anyway — zooming out bakes 3.5× MORE tiles in 5.2× LESS time,
because per-tile texels fall 16× from lod 0 to lod 2.

**Closes the latched-counter caveat below:** `debugDirtyTiles` reads **0** in every static row, so it is a
genuine per-frame count. The "constant 696" was a stale reading.

## 2026-07-26 — P0 cost attribution: reach dominates; the empty-corridor lever is struck

**Corridor vs brute**, 3 orbiting torches, zoom 1: **57.49 ms vs 87.74 ms**. The corridor's empty-space skip
already earns ~35 %, so [F2](forks.md#f2) option 4 is **struck** — there is no large win hiding in walks that
terminate on nothing.

**Reach sweep** at zoom 1 (map 512 tiles), varying reach only:

| reach (tiles) | dirty | fraction | ms | fps |
|---|---|---|---|---|
| 16 | 512 | 100 % | 55.63 | 18 |
| 12 | 512 | **100 %** | 33.71 | 30 |
| 8 | 423 | 82.6 % | **8.32** | **120** |
| 4 | 263 | 51.4 % | 8.33 | 120 |
| 2 | 141 | 27.5 % | 8.33 | 120 |

**This refutes the `work ∝ fraction × budget` model recorded above.** Reach 16 and reach 12 dirty the
*identical* 512 tiles and differ by 1.65×, so the fraction cannot be the whole story. That model fitted the
zoom sweep to 1.3 % because reach was held constant there, making the fraction the only variable — **a model
validated against a sweep that varied one input is a model about that input, not a law.** Recording the
mis-step rather than quietly replacing it: it is the same shape as I40 (a real observation, an over-general
mechanism), caught this time by testing the model against an input it had not seen.

Reach compounds three ways — walk length ∝ reach, claimed texels ∝ reach², lights overlapping each texel ∝
reach — which is why it dominates the linear levers. **[F2](forks.md#f2) resolves to option 2 (bound reach);
options 1 and 3 are deferred, not rejected.**

## 2026-07-26 — P2: reach 16 → 8 in content. The zoom cliff is gone.

Changed `&thing.light.reach` from 16 to 8 tiles on `torch` and `torch_blue`
([`content/visual/things.rd`](../../../content/visual/things.rd)), per [F3](forks.md#f3) — content stays
authoritative, the authored value was simply wrong. No renderer change, no new machinery.

Verified in-browser (content hot-update picked it up; `authoredReachTiles: 8` read back from the live prims),
3 orbiting torches:

| zoom | map (tiles) | dirty/frame | ms | fps | **was** |
|---|---|---|---|---|---|
| 1.0 | 512 | 412 | 8.32 | **120** | 43.06 ms / 23 fps |
| 0.5 | 2 048 | 693 | 8.33 | **120** | 28.34 ms / 35 fps |
| 0.25 | 8 192 | 969 | 8.33 | **120** | 8.33 ms / 120 fps |

**Every zoom now sits on the 120 fps vsync floor, moving lights included, and the dirty fraction no longer
saturates at zoom 1** (412/512 = 80 %, down from 100 %). P2's target — 3 moving lights ≥60 fps at zoom 1 — is
met with ~2× headroom over the target and no change to the shared-accumulator design.

Reach remains a live aesthetic dial with a known price: 16 → 18 fps, 12 → 30 fps, 8 and below → 120 fps.

## 2026-07-26 — P3 verification, and the move entry point closed

**Corridor↔brute identity + no-residue, at three zooms**, lights frozen at a *displaced* position after
orbiting (so the state under test is one the move path produced, not a placement):

| zoom | residue mismatches | identity mismatches | non-zero texels (both sides) |
|---|---|---|---|
| 1.0 | **0** | **0** | 39 102 |
| 0.5 | **0** | **0** | 11 836 |
| 0.25 | **0** | **0** | 2 975 |

- **Residue** compares what the incremental move path left against a `rebakeAll()` from scratch at the same
  positions. 0 mismatches ⇒ the move path leaves the map in exactly the state placement would, which is the
  direct evidence that [I1](issues.md#i1)'s stale trail is closed.
- **Identity** is corridor vs brute. Populations are large and identical on both sides, so this is **not** the
  vacuous agreement-by-emptiness that hid a total shadow outage in torch-thing P5.

**The move entry point: `Viewport.movePrim(id, x, y)` already was it.** One public call moved sprite, light
record and shadow map together; moving the prim back returned the shadow map **bit-identical** (hash
336412233 → 3373222425 → 336412233). [F1](forks.md#f1) resolves to (a) — the (b) subscriber plumbing was
proposed to route around a bug and is not needed.

## 2026-07-26 — Moving-light sweep: cost PLATEAUS at ~24 lights (the presence cap works)

Reach 16, zoom 0.5, every light orbiting, lights spread across the on-screen prims (striding the 600 nearest,
so they neither cluster nor fall off-screen), count asserted against `carriedLights.size`:

| moving lights | ms | fps |
|---|---|---|
| 8 | 53.59 | 19 |
| 16 | 91.95 | 11 |
| 24 | 135.64 | 7 |
| 32 | 146.22 | 7 |
| 40 | 162.14 | 6 |
| 48 | 152.51 | 7 |
| 56 | 164.93 | 6 |
| 64 | 156.26 | 6 |

Repeatability: 24 re-measured at **132.53 ms** vs 135.64 (2.3 %). Idle, no movers: **8.33 ms / 120 fps**.

**The headline is the plateau.** Cost climbs steeply to ~24 lights (53.6 → 92 → 135.6) and then **stops**:
32 through 64 all sit at 146–165 ms, flat within noise. That is `light_presence`'s **≤16 lights/tile cap**
doing exactly its job — once every tile is saturated, further lights are culled and cost the GPU nothing.

**This confirms the central premise of [`plan-4096.md`](plan-4096.md) by measurement rather than by reading
the code:** GPU cost is already decoupled from light count. **4096 moving lights would also land at ~150 ms**,
not at 64× that. So the target does not need a 4096× improvement — it needs **one ~9× win**, which is exactly
the gap the per-texel walk budget was sized to close (~400 M fetches → ~25 M).

Two caveats that are the reason P2 exists: the plateau assumes lights are dense enough to saturate presence
everywhere, and **the culled lights contribute no illumination at all** — so past ~24 lights this is not
"4096 lights rendered cheaply", it is 16 lights rendered and the rest discarded.

## 2026-07-27 — GPU-timed: ONE moving large-reach light costs 9.9 ms. A static one costs zero.

Wall-clock had been reading 8.33 ms / 120 fps for a single orbiting light, which says nothing — that is the
vsync floor. Measured properly with `EXT_disjoint_timer_query_webgl2` wrapped around `Viewport.tick`
(one light, reach 16, 6-tile orbit driven through `movePrim`, zoom 0.5, 88–90 samples each):

| condition | GPU ms (median) | min | max | dirty tiles |
|---|---|---|---|---|
| light **orbiting** | **10.49** | 8.74 | 13.97 | 1 120 / 2 048 |
| light **static** | **0.58** | 0.54 | 0.63 | **0** |
| **no light at all** | **0.58** | 0.54 | 1.33 | 0 |

**Two results, both important.**

**1. A cold static light is free — exactly, not approximately.** 0.58 ms with the light and 0.58 ms with no
light are the same number. This is the direct confirmation of the "write it once and you're good" question,
by GPU timer rather than by reasoning about `discard`. (Still unmeasured, and the caveat that matters: a
*caster* moving inside a cold light's reach re-dirties it via `markPrimDirty`. Cold lights are free in a
static world; nobody has measured them in one with pawns walking through it.)

**2. A single moving reach-16 light costs 9.9 ms of GPU — 59 % of a 16.67 ms frame.** The 120 fps reading was
pure vsync masking: the GPU was already ~63 % loaded by **one** light. This is the honest scale of the
problem, and it is far worse than the wall-clock sweep suggested.

Consistent with the plateau: marginal cost per light *falls* as lights are added (9.9 ms for one, ~6.7 ms/light
at eight, ~0 beyond ~24) because discs overlap, dirty tiles are shared, and presence culls the rest.

**What this implies for [`plan-4096.md`](plan-4096.md).** The ~9× the walk budget targets would take the
plateau (~156 ms at 64+) to ~17 ms — right at the 60 fps edge, and comfortable once P3 restricts casting to
the dominant few. The plan's sizing survives contact with a real GPU measurement. But it also means **there is
no headroom to spend elsewhere**: at 9.9 ms for one light, every other lever in this stream is noise.

## 2026-07-27 — F7 step 1: the shadow-edge refine is DELETED. 11.36 → 3.68 ms.

Removed the per-fine-texel `walkShadow` re-run from `LIGHT_FRAG`, plus its whole `uEdgeRefine` /
`edgeRefine` / `__edgerefine` plumbing — deleted, not left behind a toggle ([F7](forks.md#f7)).

Same fixture as the profile (one light, reach 16, 6-tile orbit through `movePrim`, zoom 0.5), GPU-timed:

| pass | before | after |
|---|---|---|
| lighting | 9.724 ms | **1.485 ms** |
| gather | 1.633 ms | 2.193 ms (unchanged code; orbit-phase variance) |
| **GPU total** | **11.36 ms** | **3.68 ms** |

**3.1× on a single moving light**, from deleting one feature. It matches [I9](issues.md#i9)'s prediction: the
refine was 9.29 ms of a 10.88 ms pass, and the pass now sits at 1.485 — i.e. what the staircase said the rest
of the shader costs.

Verified: scene renders, shadows radiate correctly from the light and fall off with distance, light pool
intact. Shadow edges are coarser — the accepted trade ([F7](forks.md#f7): *"I don't care about the
pixilation"*).

Nothing replaces it. The coarse shadow nearest-upsampled across its 8×8 block already overdraws, which is the
conservative behaviour the prim pass will finely cut.

### Clean re-profile after the deletion (2026-07-27, fresh load, same fixture)

One light, reach 16, 6-tile orbit through `movePrim`, zoom 0.5, fresh page load:

| stage | where | before | **after** |
|---|---|---|---|
| **lighting** | GPU | 9.724 | **1.462** |
| **gather** | GPU | 1.633 | 2.378 |
| **GPU total** | | **11.36** | **3.84** |
| `buildCasters` | CPU | 2.1 | **0.6** |
| `buildPresence` | CPU | 0.2 | 0.2 |
| `buildDirty` / `flush` | CPU | 0.1 / 0.1 | ~0 / ~0 |

Lighting substep staircase, re-run:

| level | includes | ms | substep |
|---|---|---|---|
| 1 | dirty gate + window mapping + `P` | 0.203 | 0.203 |
| 2 | + `receiverAt` | 1.181 | **+0.978** |
| 3 | + billboard/world normal | 1.207 | +0.026 |
| 4 | + `accumulateLights`, no shadow fetch | 1.386 | +0.179 |
| **0** | + coarse shadow lookup | **1.484** | +0.098 |

**`receiverAt` is now 66 % of the lighting pass** (0.978 of 1.484) — it was 10 % when the refine dwarfed it.
The prim pass, which deletes it, is therefore the correct next target and is worth **~1 ms of the remaining
3.84 ms GPU**.

**The gather is now the largest single GPU item at 2.378 ms**, having been 1.633 in the original profile —
that is orbit-phase/dirty-tile variance on unchanged code, not a regression, but it means the shadow walk is
now the thing to beat after the prim pass lands.

**`buildCasters` fell 2.1 → 0.6 ms** with no change to it, which is suspicious enough to note rather than
celebrate: likely a different orbit phase / dirty-rect count, not a real improvement.

### The moving-light ceiling, re-measured (same sweep as 2026-07-26)

| moving lights | before | **after** | |
|---|---|---|---|
| 8 | 53.59 ms · 19 fps | **8.30 ms · 120 fps** | **6.5×** |
| 16 | 91.95 ms · 11 fps | **8.25 ms · 121 fps** | **11.1×** |
| 24 | 135.64 ms · 7 fps | **18.58 ms · 54 fps** | 7.3× |
| 32 | 146.22 ms · 7 fps | **22.19 ms · 45 fps** | 6.6× |
| 48 | 152.51 ms · 7 fps | **22.64 ms · 44 fps** | 6.7× |
| 64 | 156.26 ms · 6 fps | **22.90 ms · 44 fps** | 6.8× |

**16 moving reach-16 lights now hold 120 fps**, where 8 previously managed 19. The 60 fps ceiling moved from
**~13 lights to ~20**. The plateau is still there (32/48/64 all ~22.5 ms — presence culling past 16/tile) but
it now sits at **44 fps instead of 6**.

**Still open from F7** — the prim pass, which is what lets `receiverAt` (1.11 ms) and the cut go too:
draw prim quads into the toroidal lightmap, sample own coverage + own normal from own frame, overwrite.

## Baseline carried in from the prior session (2026-07-26, pre-P0)

Recorded here so P0's instrumented numbers have something to sit next to. Measured at **zoom 0.25, reach 16
tiles**, 123 lights:

| condition | ms/frame | fps | dirty tiles |
|---|---|---|---|
| static | 0.72 | 120 | 696 |
| 120 of 123 moving | 72.44 | 15.2 | 5 384 |

Caveats that P0 must resolve rather than inherit:
- The **dirty counter reads a constant 696 when static and appears latched** — it may not be a per-frame
  figure at all. Confirm what it reports before any conclusion rests on it.
- Not comparable to the earlier 6.03 ms / 128-light figure: that was **reach 6**, ~7× less area per light.
- The "moving" row predates [I3](issues.md#i3) — those lights moved via the debug array that
  [primitive-graph](../2026-07-25-primitive-graph/README.md) has since deleted, so the number stands as an
  order-of-magnitude signal, not a measurement to diff against.
