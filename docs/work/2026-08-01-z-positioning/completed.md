# Completed — z positioning

_The verification log: dated entries saying what landed and **how it was checked**. Append-only._

## 2026-08-01 · P0a — the tilt exists again, and the coefficient is pinned

**`client/webgl/src/game/viewport/worldTilt.ts`** — new, and the only place the angle appears.

`WORLD_TILT_DEG = 65`, **measured from the ground plane** (0° looks along the ground, 90° straight
down). The axis is named because the same number read from the vertical is a nearly-horizontal
camera, which would invert every height in the scene — that is what the item's acceptance was
guarding against.

### The split ([I8](issues.md#i8)) — `world.y` gets nothing, `world.z` gets `tan(θ)`

Derived rather than picked, from two facts already settled:

1. **The ground planes coincide.** `P = (vec2(tileX, tileY) + inTile) * UPT` is a square grid,
   16 units on both axes, and the art draws every tile square — the 3/4 view lives in the sprites,
   not the projection. So a drawn tile of northing IS a world tile of northing, and `world.y = unit.y`
   with no term from `unit.z`.
2. **The stored elevation is the up-screen SHIFT**, not a height and not the ray's length. Forced by
   the user's own placement rule ([F6](forks.md#f6)): *"the screen-north shift IS the elevation, 1:1"*.
   A northing component of `E` on a ray rising at θ means a true height of **`E·tan(θ)`**.

`TILT_TAN ≈ 2.144507`. It is the **one** coefficient, and it is metric — it scales `world.z`, which
feeds the caster card's extent, the falloff distance and `N·L`. A wrong value there changes how
bright things are, not just where a shadow lands.

### Verified

- `npx tsc --noEmit` clean.
- `elevation → height → elevation` round-trips across the whole `u8` range, worst error
  **2.8e-14** — floating-point noise, not a modelling error.
- TS and GLSL cannot disagree: `WORLD_TILT_GLSL` interpolates `TILT_TAN` from the TS constant, the
  same construction `LIGHT_LANES_GLSL` uses next door ([F2](forks.md#f2)).

### Left honest

`screenDepth()` and `worldHeightForElevation()` currently share the `tan(θ)` coefficient. Whether
screen-perpendicular depth really wants the same factor as true height is the residue of
[I8](issues.md#i8) — but **z-ordering is invariant under any positive scale**, so if it is wrong the
ordering it produces is unchanged and only `world.z` would move. Documented at the function.


## 2026-08-01 · P0 — height is observable, and it discriminates

### The shadow path, re-read before touching it ([I1](issues.md#i1))

The plan's first item exists because two diagnoses in this stream's predecessor were given from
memory of deleted code. The live text of `occludesAt` (`records.ts`, `OCCLUSION_GLSL`):

```glsl
float h = mix(Lz, targetH, t);          // t = (C.y - L.y) / (P.y - L.y)
if (h > hTop || h < hBot) return false; // hTop = subH, hBot = 0.0  -- the card, bottom-aligned
```

`h` is the light ray's height at the caster's row, tested against a card spanning `[0, subH]`. The
machinery is real and the heights are interpolated — **I1's "heights are not populated" was correctly
withdrawn**, and [F3](forks.md#f3)'s `[z, z+H]` change is a change to `hBot`/`hTop`, not a repair.

### It discriminates — measured, not argued

Instrument: `__gather()`, which reports live shadow-texel counts and re-checks the walk against an
exhaustive brute-force reference. Every emitter's `unitZ` swept, everything else held:

| `Lz` | shadow texels | corridor walk | walk vs brute |
|---|---|---|---|
| 0 | 1016 | 503 | **0 differing** |
| 20 | **0** | 0 | **0 differing** |
| 40 | 1677 | 835 | **0 differing** |
| 80 | 1016 | 503 | **0 differing** |
| 160 | 446 | 222 | **0 differing** |
| 255 | 213 | 105 | **0 differing** |

Above the caster tops the count roughly halves per doubling of `Lz` — the shape `d·H/(Lz−H)` predicts.
**`Lz = 20 → 0 shadows` is correct behaviour, not a defect**: `content/visual/things.rd:188` documents
that a light below a caster's top makes `k = Lz/(Lz−Zt)` go negative and the caster registers nothing.
That is a *registration* gate upstream of `occludesAt`, and this sweep is the first thing to exercise it.

**Conclusion, which is what the item was for: the machinery works. The gap is the metric**
([I9](issues.md#i9)) — exactly as the plan anticipated, so P1/P2 proceed as written.

### Two real defects found and fixed on the way

**`placeLights` wrote no `unitZ` at all.** Every debug-placed light sat at `Lz = 0`, where the solve
degenerates: `h = mix(0, 0, t)` is identically 0 over a ground receiver, so `h > hTop || h < hBot`
can never reject and every caster occludes at every distance. **Every visual check in
`2026-07-31-lighting-rework` ran through this hook** — which is why its shadows were *"not remotely
correct"* while the corpus's own torches, which do author 2.5 tiles, were fine. Default is now the
corpus's documented 40 units, so the debug path and the content path agree by construction.

**`stepOrbit` dropped it again.** `writePrim` writes a *whole* record, so the orbit step's omission
put every moving light back on the floor each frame. The height now rides on `liveLights` and survives
the re-write. This is the "moving lights" path specifically, so its measurements were degenerate too.

### The elevation probe ([`__elev`](../../../client/webgl/src/game/viewport/Viewport.ts))

Answers "how high does the code think this is, and where does it draw it" in one call, because the
three numbers live in three places — elevation in the record, card extent in the definition, projection
in `worldTilt`. Verified against a known fixture: a caster declared 24 units tall reports `cardH: 24`,
and lights placed at 40 report `elevation: 40, worldZ: 85.78` (= 40 × 2.1445).

Its first version reported `cardH: 1` for a 24-unit card — it indexed the definition as
`block + rotation` where `writeDefinition` uses `block * ROTATIONS_PER_DEF + rotation`. Caught by
checking the probe against a fixture with a known answer, which is the only reason it was caught.

### `__caster` — a deterministic caster ([I14](issues.md#i14))

Places a solid `w × h` box through the real `allocPrim`/`writePrim`/`writePresence` calls. The
silhouette is skipped deliberately: `silhouetteHit` returns its `offPage` argument for a definition
past the two bound atlas pages and `occludesAt` passes `true` there — the documented *"conservative
answer (casters: the solid box)"* — so parking the def on page 3 yields an exact rectangle. A
disagreement is then geometry, never art.

### Corrected mid-task

I reported "nothing casts and nothing receives" from a record dump taken **before the scene finished
syncing**, and nearly filed it. With the scene loaded there are **525 casters and 1033 shadow texels**,
and `firstFrameNonZeroTexels: 0` — which I also nearly read as current state — is a startup statistic.
The lesson is the stream's own rule: this codebase populates asynchronously, so a count of zero means
"not yet" until proven otherwise.


## 2026-08-01 · P0b — the channel relayout

`definition_index` moved from GREEN to BLUE's clean low 16 bits; `seed` and `rotation` moved to GREEN
to pay for it; `u4 layer` became **`u4 fine.z`**.

```
G  u8 unit.z (24-31) | u4 fine.x (20-23) | u4 fine.y (16-19)
   u4 fine.z (12-15) | u8 seed (4-11)    | u4 rotation (0-3)
B  u2 cast (30-31) | u2 receive (28-29) | u2 emit (26-27)
   u4 reach (22-25, biased) | u6 intensity (16-21) | u16 definition_index (0-15)
```

### The prim and the definition now diverge

They shared one `packTypeLanes`. They cannot any more: a definition has no `definition_index` to
store, and it still needs `seed` where it is because `silhouetteHit` reads that lane as
`pxPerUnit × 8`. So `packPrimTypeLanes` is new and `packTypeLanes` stays for definitions.

**"A prim copies the definition's BLUE" now means field-wise** — which it always was in practice.
`RecordSync` passes `castType`/`receiveType` from the resolved definition into `writePrim`, and each
record packs from the field NAMES independently; nothing ever copied the raw word. That is exactly
why the split is safe, and `VARIABLES.md` now says so instead of implying a word copy.

### `layer` was write-only

`recordSync` set it to the pawn part slot and **no shader ever read it back** — confirmed by grepping
every lane read before removing it. Retiring it is therefore free, and there is a small symmetry in
it: the lane that recorded *which* part a prim was now records *how high* it is. Removed from
`PrimFields` rather than left as an ignored field, so the compiler found all three writers.

### Verified — a controlled A/B, same protocol both sides

`git stash` of the source, same page, same fixed protocol (sync via one `tick()`, then `__lights(4)`
at the corpus's 40 units, then `__gather()`):

| metric | OLD | NEW | Δ |
|---|---|---|---|
| **`occlusionDiffering`** | **0** | **0** | **0** |
| `slotsCompared` | 131072 | 131072 | 0 |
| `castersRestored` | 521 | **524** | +3 |
| shadow texels | 4242 | 4253 | +0.26% |
| `corridorWalk` | 3696 | 3702 | +0.16% |
| `gateSelectivityPct` | 4.85 | 4.86 | +0.01 |

**Not bit-identical, and the reason is in the table**: `castersRestored` differs by 3, so the two runs
did not have the same scene — records populate asynchronously ([I14](issues.md#i14)) and a load syncs
what it has got. Every other figure moves by less than the caster count does. The invariant that
matters holds on both sides: the walk agrees with an exhaustive brute-force reference on **0 of
131,072** slot comparisons.

Also checked in-browser under the new layout: `__records().roundTrip.exact` true (every field writes
and reads back), all six lane assertions pass, and `__elev` decodes a fixture caster's `block: 3` and
`cardH: 24` from the values it was declared with.

### Two bugs caught by verifying rather than assuming

- **The probe read stale lanes.** It reported `block: 0` for a caster written with `block: 3` — it was
  still reading `definition_index` from GREEN. Only visible because the fixture had a known answer.
- **The `__gather` self-test round-trip dropped the fine lanes.** It saves and restores every caster
  twice, and `writePrim` writes a *whole* record — so the omission was nudging every caster onto a
  whole-unit position for the duration of the check. Pre-existing; fixed while the file was open.


## 2026-08-01 · P1 — the caster card starts at its own height ([F3](forks.md#f3))

```glsl
// was: float hTop = float(subHi), hBot = 0.0;
float cElev = primElevation(rec);
float hBot = cElev, hTop = cElev + float(subHi);
```

Both branches of `occludesAt` — the x-aligned card and the `cast_type 2` perpendicular one. There is
only ONE definition to change: the refine calls the same `occludesAt` (`lightPass:232`), so
[F2](forks.md#f2)'s "one rule, one place" already held here and did not need building.

`silhouetteHit`'s `fracY = (hTop - h) / subHi` needed no change — it normalises *down the card*, which
is `subHi` tall wherever the card sits.

### The ground case is unchanged

At `cElev = 0` the new expressions are the old ones exactly. Measured, same protocol, same run
(`castersRestored` identical at 524, so directly comparable):

| metric | pre-P1 | post-P1 | Δ |
|---|---|---|---|
| `occlusionDiffering` | 0 | 0 | **0** |
| `castersRestored` | 524 | 524 | **0** |
| `corridorWalk` | 3702 | 3702 | **0** |
| shadow texels | 4253 | 4254 | +1 (0.02%) |

### The walk stays exact with elevated casters

`__gather()` re-checks the corridor walk against an exhaustive brute-force reference. Casters given
mixed elevations, 131,072 slot comparisons each time:

| caster elevations | occlusionDiffering | shadow texels | corridorWalk |
|---|---|---|---|
| all 0 | **0** | 4254 | 3702 |
| 0/4/8/16 | **0** | 4254 | 3702 |
| 0/8/24/48 | **0** | 5854 | 5419 |
| all 32 | **0** | 2477 | 2043 |

**Zero differing at every mix** — that is the acceptance. And elevation visibly reshapes the field:
raising every caster 32 units cuts the shadow texels to 58% of the ground case, while a mixed set
raises them to 138%, which is the expected behaviour when some cards lift clear of the light rays and
others start intercepting rays that used to pass over them.

### Unexplained, and left that way

**`0/4/8/16` produced numbers byte-identical to all-zero** — 4254 texels, 3702 walks, 41
identityDiffering. The writes definitely landed: a probe immediately afterwards shows 6 casters at
each of 0, 4, 8, 16 with card heights of 13 and 24 units. So small elevations changed literally
nothing while larger ones changed a great deal.

I do not have an explanation, and the plausible ones (the self-test's own save/restore, a re-sync
between write and measure) are guesses. **Recorded as [I17](issues.md#i17) rather than explained** —
this stream's predecessor lost time to three confident diagnoses that turned out to be invented, and
the acceptance that matters (0 differing) does not depend on resolving it.


## 2026-08-01 · Correction — the texel counts quoted above are noise ([I17](issues.md#i17))

Re-running identical inputs A/B/A shows the shadow-texel count varying by ~12% with the records held
constant (5854 → 5854 → 5131 for the same all-zero elevations). The shadow buffer is ping-ponged and
incremental — the `incumbent` tier *is* carried-over shadows — so `nonZeroBefore` reports accumulated
state rather than a function of the current scene.

**Struck from the evidence above:** P0b's "shadow texels 4242 → 4253 (+0.26%, tracks caster count)"
and P1's "shadow texels 4253 → 4254 (+1)". Neither number means anything.

**Both conclusions still hold, on evidence that does not depend on it:**

- **P0b** — `roundTrip.exact` true, all six lane assertions pass, `__elev` decodes a fixture's
  declared `block: 3` and `cardH: 24`, and `occlusionDiffering = 0` over 131,072 comparisons.
- **P1's ground case** — needs no measurement: at `cElev = 0`, `hBot = cElev` and
  `hTop = cElev + subHi` reduce to `hBot = 0.0` and `hTop = subHi`, which is the replaced code exactly.
- **P1's mixed heights** — `occlusionDiffering = 0` at every elevation set, which is the acceptance
  as written.

**And I17's original claim is withdrawn entirely.** There is no "small elevations do nothing" effect;
there was an unreproducible metric. That matters for P3, which I had flagged as at-risk on the
strength of it — it is not.


## 2026-08-01 · Correction — P0a's coefficient was wrong ([I8](issues.md#i8))

`worldTilt.ts` said `world height = elevation × tan(θ)`. Both halves were wrong:

- **The factor is `sin(θ)`.** `content/visual/things.rd` still documents the retired
  `shadowGather.ts`'s rule — *"the caster's card top (elevation `Zt = H·sin(WORLD_TILT)`)"*, worked
  through as *"a 2-tile tree tops out at 32·sin55° ≈ 26 units"*.
- **It applies to DRAWN extents, not to the stored lane.** A light's `unit.z` is *already* a world
  height (`SquareCache.height` is "world px above the ground plane", stored straight). What needs
  converting is a card's drawn height and a point part way up it.

I derived `tan(θ)` from a model of what `unit.z` *ought* to be rather than reading what the code and
the corpus already put there — in the very phase whose first item is "re-read before planning against
it". `TILT_SIN` is now the height factor; `TILT_TAN` keeps only `screen.z`, the ordering depth, where
scale-invariance made it harmless.

**This does not disturb P1.** Its change is `hBot/hTop` relative to the caster's own elevation, and
that is right in whatever unit the terms share. What P1 inherited — and P2 must fix — is that they do
**not** currently share one: `Lz` is world, `subHi` and `targetH` are drawn. That is [I9](issues.md#i9),
now with a coefficient and a source.


## 2026-08-01 · F10 — `definition_data`'s retired lanes closed, nothing built

[I16](issues.md#i16) established the plan item was unbuildable as written (`seed` is live for
`silhouetteHit`'s `pxPerUnit × 8`, `rotation` for `debugDefinition`). Decided: **close it.**

The item existed to free bits, which is worth doing only under bit pressure. The prim record had
pressure — `definition_index` needed 16. The definition record has none: nothing is waiting to move
into its BLUE. Retiring `layer` there would leave a hole, not make room. Recorded as
[F10](forks.md#f10).


## 2026-08-01 · P2 — one meaning for `unit.z`, and the tilt enters where it belongs

### F9(b): the lane means a DRAWN shift, converted once at the writer

```ts
unitZ: L ? Math.min(255, Math.round(drawnForWorldHeight((L.height / SQUARE) * UNITS_PER_TILE))) : 0,
```

A light's authored height is a **world** height; the lane stores a **drawn** shift. One conversion,
in `RecordSync`, and every downstream reader compares like with like.

**Content-visible:** every light is now `1/sin 65°` = **1.103× higher**, so every shadow is ~10%
shorter. The corpus's 40-unit torch contract reads as **44 drawn units** and still clears a 2-tile
tree comfortably. One line to revert if the 10% is unwanted ([F9](forks.md#f9)).

### The payoff — `occludesAt` needed NO conversion

The plan's item says *"give `occludes()` the screen→world transform it has never had"*. **It does not
need one.** Once the lane means one thing, every term in the height test is a drawn quantity —
`Lz`, `cElev`, `subHi`, `targetH` — and they compare correctly as they are. Deviation recorded in
[`deviations.md`](deviations.md).

The tilt belongs where a height meets a **horizontal** distance, which is `N·L`:

```glsl
vec3 ldir = normalize(vec3(Lpos.x - Puse.x, worldHeightForDrawn(Lz2 - targetH), Puse.y - Lpos.y));
vec3 ldir = normalize(vec3(Lpos.x - P.x, P.y - Lpos.y, worldHeightForDrawn(Lz2)));   // ground frame
```

Two lines. Previously the height term went in raw, so every light's direction was biased toward the
horizontal by an amount growing with its height — [I8](issues.md#i8)'s prediction, now fixed.

### Verified

| check | result |
|---|---|
| `occlusionDiffering`, scene elevations | **0** / 131,072 · 524 casters |
| `occlusionDiffering`, mixed 0/6/12/24 | **0** / 131,072 · 524 casters |
| `occlusionDiffering`, all at 44 | **0** / 131,072 · 524 casters |
| `glError` | 0 |
| `drawn → world → drawn` over the whole `u8` range | worst **2.8e-14** |
| debug light default | `40` world → **`44`** drawn, read back from the record |
| TS/GLSL agreement | `WORLD_TILT_GLSL` interpolates `TILT_SIN` from the TS constant — cannot drift |

**Ground case unchanged**: at elevation 0, `worldHeightForDrawn(0) = 0`, so both `ldir` expressions
reduce to what they were. No measurement needed — and per [I17](issues.md#i17) a texel count would
not have been evidence anyway.

### P0a item 1 closes here

Its acceptance was *"one source, read by BOTH the record writer and the shadow transform"*. Now true:
`RecordSync` reads `drawnForWorldHeight`, `lightPass` reads `worldHeightForDrawn`, both out of
`worldTilt.ts`, with the reference axis named. The deviation logged when P0a was built is resolved.


## 2026-08-01 · P2b — the coordinate systems separate, and the head lands on the body's footprint

**The record now holds GAME coordinates** — where a prim physically stands, not where it is drawn.

### A carried piece stands where its CARRIER stands

Adding the elevation back to the drawn row was not enough. Measured, head and body still sat 4 units
apart: each prim derives its ground row from its **own** opaque bbox, and a head's art bottom is the
bottom of the head, not the pawn's feet.

So the carrier's ground row is resolved in a pre-pass and pieces adopt it. The vertical gap between a
piece's own art bottom and its carrier's row **is** the piece's elevation — derived, not authored:

```ts
const carrier = p.carrierOf !== undefined ? carrierGround.get(p.carrierOf) : undefined;
const ay   = carrier ? carrier.ay : ownGround + (p.elevation ?? 0);
const elev = carrier ? Math.max(0, carrier.ay - ownGround) : (p.elevation ?? 0);
```

`ax` deliberately stays the prim's own: an `offset.x` part genuinely *is* at a different ground x.

### The result — read off the live pawn

| prim | `unitX` | `unitY` | elevation | `screenY` | card top (world) |
|---|---|---|---|---|---|
| 525 — body | 1528 | **911** | 0 | 911 | 13.60 |
| 526 — head | 1528 | **911** | **9.125** | **901.875** | 22.72 |

**Same ground position, differing only in `unit.z`** — the acceptance verbatim. `screenY` is
`911 − 9.125 = 901.875`, so the head still draws exactly where it did: the drawing is unchanged and
only its *meaning* moved.

### `targetH` fixed itself

[F8](forks.md#f8) wrote the fix as `E + (screenBase − P.y)`. With game coordinates
`screenBase = groundY − E`, so it collapses to `groundY − P.y` — the line already there. **The
expression did not change; `primPos` returning a ground row made it correct.** Before this, a head's
texels were shadow-tested as though the head stood on the floor.

### Verified

| acceptance | result |
|---|---|
| head and body write the same `unit.x/y` | **1528 / 911 both**, elevation 0 vs 9.125 |
| presence keys on game coordinates | both in tile **(95, 56)**, both in that tile's presence slots `[525, 526]` |
| no caster read converts per-test | the conversion is once per texel at `ldir`; `occludesAt` reads records directly |
| the walk stays exact | `occlusionDiffering` **0** / 131,072 · 524 casters · `glError` 0 |


## 2026-08-01 · P3 — the two shadows became one, verified on screen

**The number this stream exists to move.** A/B on the live pawn, same light, same frame, zoomed:

| | head's record | shadow |
|---|---|---|
| **before** — head is its own root | `unitY 901`, elevation 0 — **10 units north of the body** | a **broad doubled mass**, far wider and taller than the pawn, sweeping up-right: two silhouettes from two footprints |
| **after** — head adopts its carrier | `unitY 911`, elevation **9.125** — the body's row | **one narrow shadow** matching the pawn's silhouette, anchored at the feet |

The "before" was produced honestly, not from memory: clearing `carrierOf` on the live head prim and
re-syncing puts the record back to two footprints, which is exactly the old behaviour. Restoring it
returns `unitY 911 / elevation 9.125`, `occlusionDiffering` **0**, 524 casters.

It also settles one of [lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13)'s
complaints in passing — *"the shadows aren't anchored at the base of our billboards"*. In the after
capture the shadow starts at the feet.

### What is NOT done: the corpus still authors `offset.y`

[F5](forks.md#f5) says the head should author `offset.z` and lose `offset.y`. **Not made**, and the
reason is worth stating: **the alignment does not depend on it.** The carrier link derives the
elevation from geometry, so the head aligns today with its existing `-0.87 &head.offset.y`.

The DSL half is built but cannot be *exercised* here — `offset.z` is read by `loader.rs` and exported
by `shared/wasm`, both of which need a cargo build, and **there is no cargo toolchain in this
environment**. Editing `content/visual/pawns.rd` to use `offset.z` now would break the head's drawing
until someone rebuilds the wasm, because the shipped `shared/pkg/resonantdust_shared_bg.wasm` would
parse a field it does not know and drop the `offset.y` that currently does the work.

**Recorded as a deviation** with the exact post-rebuild change.


## 2026-08-01 · P4/P5 — lighting by the tile a thing stands on, cost, and what this generalises to

### The blit already lights a billboard by where it STANDS ([lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 4)

No change needed, and P2b is why. `__zprobe(95, 56)` on the pawn's tile:

```
receivers: [ 0 → 3033 texels (ground), 525 → 617 (body), 526 → 446 (head) ]
```

Both parts own lighting texels, so each is lit through the receiver mechanism at its **card's plan
position** (`Puse = (P.x, baseY)`), and since P2b `baseY` is the true ground row rather than the drawn
one. The blit samples the lightmap at the drawn pixel, but the value at that texel was computed for
the card standing where it stands. Receiver self-test alongside: 2,097,152 texels sampled, 219,666
with a receiver, 110 distinct owners, `ownersThatAreNotReceivers` **0**, `glError` 0.

### Cost — elevation is free, and flattening it is *expensive*

The plan wanted a comparison against the pre-stream baseline. **That baseline is not comparable**: the
lighting-rework figures (2.79 ms static, 3.11 ms with 16 moving lights) were taken at
`?focus=100,50`, which holds **zero casters** — they measured almost none of the shadow system.

So the honest measurement is this stream against itself: same scene, same 16 lights at reach 16,
elevations live vs flattened to 0.

| | frame |
|---|---|
| elevation live | **10.35 ms** (min 10.29) |
| every elevation zeroed | **13.15 ms** (min 11.08) |

**Elevation is 2.8 ms FASTER.** Not a surprise once stated: zeroing every height puts the lights back
at `Lz = 0`, where the height test cannot reject and every caster occludes at every distance — P0's
degenerate case, arrived at again from the performance side. Elevation adds arithmetic to existing
passes and removes work from the walk.

### What this buys beyond the pawn's head

- **A wall torch** — bracketed above head height. It now casts from the wall's ground footprint, not
  from a phantom position a metre north of the wall, and its light direction is correct because the
  height term is converted before it meets the ground offsets.
- **A carried item** — a tool, a lantern, a pack. Elevation comes from the carrier link for free: any
  piece with `carrierOf` adopts its carrier's footprint and derives its own height from geometry, so
  new carried art needs **no** authored number to shadow correctly.
- **Any elevated caster at all** — a bird, a shelf, a sign. The caster card spans `[z, z+H]`
  ([F3](forks.md#f3)), so a thing off the ground casts a shadow displaced by the geometry rather than
  one starting at the floor.

The general rule: **height is authored once and everything else is derived.** The class of bug this
removes is the one where a drawn offset was also, silently, a claim about where something stood.


## 2026-08-01 · P4 — the part-ordering rule, and a correction to F4

**Correction: `screen.z` cannot be the part-ordering key**, and I recorded earlier that it could.

`facingDepth(depth, facing)` negates the authored depth when a pawn faces north, so a head at `+1`
draws over the body from the front and **under** it from behind. `screen.z = tan(θ)·elevation` is
facing-independent — a head is the same height whichever way the pawn is turned — so an ordering key
derived from height loses the from-behind case.

`screen.z` is still a sound ordering metric for *independent objects* at different heights, which is
what it was proposed for. It is not a substitute for authored part order **within one object**.

So [F4](forks.md#f4)'s original choice (c) stands, and its condition is now satisfied: the rule is
written at `slotZ` in `MoverLayer`, where someone would go to change it, together with why elevation
is not the key. That matters more since P2b — head and body now share a base row, so `zRowBase` is
equal for both and this expression *is* what separates them. It was true by accident of iteration
order before; it is stated now.

## 2026-08-01 · P5 — the A/B set

| | before | after |
|---|---|---|
| **head aligned** | head's record at `unitY 901`, elevation 0 — 10 units north of the body | `unitY 911`, elevation **9.125** — the body's row |
| **one shadow** | a broad doubled mass, wider and taller than the pawn, sweeping up-right | one narrow shadow matching the pawn's silhouette, anchored at the feet |
| **ground prim unchanged** | — | at elevation 0 every expression this stream touched reduces to the code it replaced (`cElev = 0` ⇒ `hBot = 0, hTop = subH`; `worldHeightForDrawn(0) = 0`) |

Both captures are real frames from the same session, same light, same tile — the "before" produced by
clearing `carrierOf` on the live head prim and re-syncing, not recalled. The ground-prim row is
argued rather than measured on purpose: it is an algebraic identity, and per [I17](issues.md#i17) a
texel count would not have been evidence anyway.


## 2026-08-01 · P4 — one word, one meaning ([F1](forks.md#f1))

Now that `unit.z` means **height**, every neighbouring `z` that meant **draw order** was a trap. F1
predicted the failure exactly: *"how someone later reads `slotZ` as elevation and spends a day on it"*.

| was | is |
|---|---|
| `slotZ` | `slotOrder` |
| `PAWN_Z_BASE` | `PAWN_ORDER_BASE` |
| `SLOT_DEPTH_Z` | `SLOT_DEPTH_ORDER` |
| `SLOT_ORDER_Z` | `SLOT_INDEX_ORDER` |
| `zRow` / `zRowBase` | `orderRow` / `orderRowBase` |
| `thingZ` | `thingOrder` |

**Scoped deliberately.** `zIndex` (47 uses, 13 files) was left alone: it is standard graphics
vocabulary for draw order and nobody reads it as a height. `MoverPart.depth` was left alone too, and
for a better reason than habit — since this stream, `screen.z` **is** a depth perpendicular to the
screen, so `depth` naming a view-axis quantity is now literally accurate rather than merely
conventional.

A comment at each renamed definition says *why*, so the next person does not undo it.

### Verified — a rename must change nothing

| check | result |
|---|---|
| `occlusionDiffering` | **0** / 131,072 · 524 casters |
| `everyShadowVanished` when cast types cleared | true |
| `ownersThatAreNotReceivers` | 0 |
| `roundTrip.exact` | true |
| head and body footprint | **same** `unit.x/y`, head elevated |
| `glError` | 0 |


## 2026-08-01 · P3 — the corpus authors a HEIGHT ([F5](forks.md#f5))

The deviation logged earlier — *"deferred, no cargo toolchain"* — was **wrong about the environment**.
`bin/rd build shared` builds the wasm bundle **in docker** (`shared/compose.yml`'s `wasm` service),
and the `rd-sim-builder` image was already present. No host cargo is needed and none ever was; I
checked for `cargo` on `PATH`, found nothing, and stopped one step short of the build script that
exists precisely because there is no host cargo.

```
-0.87 &head.offset.y set        →        0.87 &head.offset.z set
```

Both head slots — female and male — in `content/visual/pawns.rd`.

### The build caught what I had missed

`rustc` rejected the first attempt: `missing field 'elevation' in initializer of 'VisualPart'` at
`shared/wasm/src/lib.rs:401` — a *second* construction site, the default slot for a kind with no
parts list. Exactly the kind of thing a real build finds and a grep does not.

### Verified — the drawing is unchanged, the meaning is not

| | before (`offset.y = −0.87`) | after (`offset.z = 0.87`) |
|---|---|---|
| DSL delivers | `offY −0.87, offZ 0` | **`offY 0, offZ 0.87`** |
| head draws at | `y = 7120.64` | **`y = 7120.64`** — identical |
| head's record | `unitY 911`, elevation 9.125 | `unitY 911`, elevation 9.125 |
| body's record | `unitY 911`, elevation 0 | `unitY 911`, elevation 0 |

`occlusionDiffering` **0** / 131,072 · 524 casters · `glError` 0.

**Two sources, each authoritative for its own thing** — worth stating because it looks like
duplication and is not. The DSL's `offset.z` drives the **drawing** (where the head's frame centre
goes). The carrier link drives the **shadow elevation** (the gap between the head's art bottom and
the pawn's feet). They are different quantities: `offset.z` locates a frame centre, the record needs
the card's *bottom*. Deriving the second from the first would reintroduce exactly the art-dependent
fudge this stream removed.

## 2026-08-01 · Stream complete — 30/30

**Body and head cast ONE aligned shadow**, verified on screen against a real before-image.

Secondary: elevation costs **nothing** — it is 2.8 ms *faster* than flattening, because flat heights
put every light at `Lz = 0` and degenerate the occlusion solve.

Everything else this stream found is in [`issues.md`](issues.md); the two corrections that changed
conclusions are [I17](issues.md#i17) (texel counts are not reproducible — never quote them) and
[I8](issues.md#i8) (the tilt factor is `sin`, on drawn extents only).
