# Issues — z positioning

_Problems hit, candidate solutions, which we chose and why. **Open issues only** — solved ones are
removed once their resolution is logged in [`completed.md`](completed.md)._

## I1 — WITHDRAWN: I read stale code; lights DO have height {#i1}

I claimed `unitZ` was never written, so `Lz` was 0 and the caster height test collapsed to `0 <= H`.
**False.** `recordSync.ts:194` writes it from the authored light height:

```ts
unitZ: L ? Math.min(255, Math.round((L.height / SQUARE) * UNITS_PER_TILE)) : 0,
```

And `occludesAt()` does a proper height interpolation along the ray — `h = mix(Lz, targetH, t)`,
against a bottom-aligned card `[0, subH]`, with the silhouette sampled at `(hTop - h) / subH`. There is
also a `cast_type 2` branch handling n/s side-frame casters.

I was describing code I wrote earlier in the same session; `lighting-visual` and
`lighting-correctness` ran after it and rewrote exactly these parts. **I diagnosed from memory instead
of re-reading** — the identical failure I had recorded in
[lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) and then repeated twice more in
conversation.

**The stream's premise survives, narrowed.** See [I9](#i9).

## I2 — `z` already means draw order in this code {#i2}

Four existing `z`s, none of them height:

| symbol | means |
|---|---|
| `slotZ()` | the painter's key: `PAWN_Z_BASE + zRow + facingDepth·SLOT_DEPTH_Z + i·SLOT_ORDER_Z` |
| `MoverPart.depth` | *"draw-order offset along the view's depth axis"* |
| `prim.zIndex` | sprite ordering |
| `zdepth-world` | the composite carrying that key |

Introducing a fifth `z` that means **height** is how someone later reads `slotZ` as elevation. The
stream says `elevation` upstream of the record ([F1](forks.md#f1)) and P4 renames the draw-order ones,
so the collision closes from both ends.

## I3 — "Where does it stand" is one question, asked in two broken places {#i3}

[lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 4: the blit samples the
lightmap at `vWorld = aPosition`, so a billboard takes the light of the ground it is **drawn** over
rather than the tile it **stands** on. Up to a full tile of error at `SQUARE` 128, scaling with sprite
size.

That is the same question this stream is answering for shadows — *what is this object's ground
position, as distinct from where it is drawn* — so the two are fixed together in P4 rather than
separately. Elevation makes the distinction explicit for the first time: before it, "drawn position"
and "ground position" were the same field, and there was no way to write the fix.

## I4 — RESOLVED: there is no draw-side constant ([F6](forks.md#f6)) {#i4}

_The user settled the coordinate model across four diagrams. The screen-north shift IS the elevation,
1:1; the tilt lives only in `unit.z`'s decomposition and in the shadow transform. Kept for the
reasoning, since it records why fitting the constant to the current sprite offset would have been the
wrong method — it would have reproduced today's drawing exactly and hidden the error in the shadow._

**Original entry:**


P2 says "derive the drawn offset from `worldTiltDeg`". **I wrote that without knowing which function
of the tilt it is**, and the codebase offers two plausible ones already in use:

| existing constant | value at 65° | what it is used for today |
|---|---|---|
| `elevK = sin(tilt)` | 0.906 | the receiver-elevation gain — shadows climbing a billboard |
| `nsInv = 1 / cos(tilt)` | 2.366 | un-foreshortening N–S distances |

My "sanity check" in the README — that `head.offset.y = -0.87` implies a head height near 1 tile —
is **circular**: I inferred the height from the offset using the constant I was trying to confirm. It
is consistent with `sin`, and that is all it is.

**P2 must derive this from the projection, not fit it to the current offset.** Fitting reproduces
today's drawing exactly and would hide the error in the shadow, which is the one thing this stream
cannot afford — the whole point is that the drawn position and the shadow position stop being tuned
independently. The test that settles it: place a caster at a known height under a light at a known
height and check the shadow length against the geometry, *not* against the old sprite position.

## I5 — RESOLVED by a channel relayout (user, 2026-08-01) {#i5}

`u8 unit.z` alone quantises height to 1/16 tile with no sub-unit lane, against today's float
`offset.y`. The user's fix moves lanes rather than accepting the snap:

```
GREEN  u8 unit.z | u4 fine.x | u4 fine.y | u4 fine.z | u8 seed | u4 rotation   = 32
BLUE   u2 cast | u2 receive | u2 emit | u10 intensity | u16 definition_index   = 32

definition_data BLUE   u2 cast | u2 receive | u2 emit | u10 intensity | u16 reserved = 32
```

`u4 layer` pays for `u4 fine.z`. **Verified before agreeing: no shader reads the layer lane.** Its only
uses are the write itself and `presence`'s sort — and that sort reads a value the *caller* supplies,
not one fetched back from the record. It is write-only.

`definition_data` loses `layer`/`seed`/`rotation` to `u16 reserved`: seed and rotation are per-prim
with a definition default, and with layer gone the definition has no use for the lane.

**Range.** `u8` units = 255 units = **15.94 tiles ≈ `ZONE_DIM`**, and `frame.span` caps a prim at 16
tiles — so a prim can be up to **16 tiles cubed**, symmetric in all three axes.

`fine.z` is `u4` px within a unit, matching `fine.x`/`fine.y`. **SQUARE is 128, so `UNIT` is 8 px and
the fine lanes only ever use 0–7 — half the nibble.** They are `u4` for headroom, per the design's own
reasoning: `SQUARE` is documented as *"128px, 64px minimum, 256 maximum"*, and at that maximum a unit
is 16 px and needs the fourth bit. The extra bit is a `SQUARE` dial, not a current requirement.

**One doc line needs updating with it**: *"BLUE and ALPHA are copied from the definition"* stops being
literally true — BLUE's low 16 bits become the prim's own definition index, and `seed`/`rotation` move
to GREEN. The copy rule survives, it just describes different lanes.

## I6 — WITHDRAWN: I was wrong about one caster per slot {#i6}

I claimed that body and head sharing a footprint would contend for one stored caster, so a pawn would
cast one part's shadow and lose the other's. **That is wrong, and the user corrected it.**

The gather stores one caster **per UNIT per light** — not one per pawn. Each unit independently keeps
whichever caster occludes *it*, so a unit shadowed by the body stores the body, a unit shadowed only
by the head stores the head, and **the union is represented naturally** across units. There is no
contention at the level I claimed.

The real residual is much smaller and sub-unit: within a single unit that is *partially* covered by
both, the refine tests only the stored caster's silhouette, so a pixel covered by the other one alone
renders lit. As the user put it — the error is the fraction of a unit partially occluded by one part
and not the other, on ground that is often already shadowed anyway.

**Left as a thing to look at during P3, not a design change before it.** I escalated a sub-unit
sampling artefact into an architectural blocker by reasoning about the storage instead of about what a
unit actually does.

## I7 — RESOLVED, and worse than a wrong value: the tilt DOES NOT EXIST {#i7}

I asked whether the angle was the diagrams' **55°** or the code's **`worldTiltDeg = 65`**. The user:
*"whatever use 65 then. We changed to 55 at one point, apparently you changed back."*

**Neither. Grepped: `worldTiltDeg`, `__tilt()` and the `uTilt` uniform do not exist.** They lived in
`shadowGather.ts`, which `2026-07-31-lighting-strip` P2 deleted — I removed the dial with the system
and then quoted 65 from memory of the old code. My own planning language ("off the live tilt",
"`__tilt()` keeps re-tilting coherently") describes machinery I had already deleted.

**This explains the shadow bug more precisely than "the transform is missing".** `occludes()` has no
angle term *anywhere* — it is not using the wrong tilt, there is nothing for it to use. The transform
was never written because the parameter it needs went with the strip.

**Resolution: the value is 65°, and it is reintroduced as a live parameter, not a literal.** The
user's framing is the operative one — *"regardless of world angle our math works"* — so the angle is a
runtime input to the transform, and 65 is just today's setting. It needs:

- one authoritative source (a constant plus a `__tilt()`-style dial), since it is now consumed by the
  record writer (`unit.z`'s decomposition) **and** the shadow transform
- both to read the same source, or the sprite and its shadow disagree about the world — the failure
  this stream exists to remove

A note for whoever re-adds it: the old dial also re-derived `elevK = sin`, `nsInv = 1/cos` and the
normal pitch together, so the whole geometry re-tilted as one. Anything less than that is a partial
dial that lies when turned.

## I8 — RESOLVED: the factor is `sin(θ)`, and only DRAWN extents take it {#i8}

Found in the corpus, not derived. `content/visual/things.rd` still carries the contract from the
retired `shadowGather.ts`:

> *"`shadowCover` projects the caster's card top (**elevation `Zt = H·sin(WORLD_TILT)`**) from the
> light onto the ground; when the light sits BELOW that top, `k = Lz/(Lz−Zt)` goes negative and the
> caster returns 0 — no shadow at all, silently. **A 2-tile tree tops out at 32·sin55° ≈ 26 units**,
> so a light must clear that to cast against anything."*

**Two different quantities share the `unit.z` lane, and only one converts:**

| quantity | is | conversion |
|---|---|---|
| a **light's** height | already world — `SquareCache.height` is *"world px above the ground plane"*, stored straight by `RecordSync` | **none** |
| a **drawn extent** — a card `subH` tall, a point up it | screen | **× sin(θ)** |

### This corrects P0a

P0a recorded `tan(θ)` and I built `worldTilt.ts` on it. Wrong twice over: the factor is `sin(θ)`, and
it does not apply to the lane I applied it to. I reasoned from the coordinate model in the abstract
instead of reading what the code and corpus already stored — the same failure this stream's first
plan item exists to prevent, committed in the phase that item belongs to.

`tan(θ)` keeps one job: **`screen.z`, the z-ordering depth**, which is only ever compared, never
measured. Sorting is scale-invariant, so that use was never at risk.

### What it means for P2 — this IS the metric gap ([I9](#i9))

`occludesAt` now compares, in one expression:

- `Lz` — a light's **world** height
- `hTop = cElev + subHi` — where `subHi` is a **drawn** extent (P1)
- `targetH = max(0.0, baseY - P.y)` — a **drawn** screen difference

So world and drawn quantities are being compared directly, with no `sin(θ)` anywhere. **That is the
gap P0's sweep pointed at**, now with a coefficient and a source. P2's change is to convert the drawn
terms: `hTop = cElev + worldHeightForDrawn(subHi)` and likewise for `targetH`.

**One open question for the user**, since it changes a shipped look: the corpus comment computes with
**sin 55°**, while `WORLD_TILT_DEG` is **65°** per their instruction (*"whatever use 65 then. We
changed to 55 at one point"*). At 55° a 2-tile tree tops out at 26.2 units; at 65°, 29.0. Every
shadow length shifts by ~11%. **Recommend 65°** — it is the explicit instruction and the newer of the
two — with the note that the 40-unit torch contract was chosen against the 55° figure and still
clears the 65° one.

## I9 — The real gap: heights are ISOTROPIC with ground distance {#i9}

Replaces the wrong diagnosis in [I1](#i1) and [I8](#i8).

`Lz`, `targetH` and `hTop` are all in **units** — the same units as x/y ground distance — and
`h = mix(Lz, targetH, t)` interpolates them as if commensurate. **One unit of height is treated as one
unit of ground distance.**

That is a projection choice, and an internally consistent one, which is why the shadows read as
plausible rather than obviously broken. It is also the thing the user's diagrams identify as wrong: the
ground is foreshortened relative to the screen vertical, so a unit of height and a unit of ground
distance are **not** the same world length. The missing relationship is exactly the cyan line —
`tan(world_angle)`.

**This is a metric correction in the height comparison, not a transform written from nothing.** The
ray maths, the bottom-aligned card, the silhouette sampling and the n/s branch are all already there
and already coherent. What they lack is the scale factor between the two axes they compare.

Which also means the **drawn** side needs nothing: drawing is flat and screen-aligned, with the
obliqueness baked into the art.


## I14 — The live scene cannot supply a caster on demand — MITIGATED {#i14}

Verifying anything about a shadow needs a caster whose geometry is known. The live scene resists that
in three separate ways, and each one cost time before it was identified:

- **The standard fixture region is empty.** `?focus=100,50` holds 2965 map prims and **zero** with a
  texture — bare terrain. Nothing there can cast, so every shadow check run against it was checking
  nothing. (`?focus=95,56` does have content, including corpus torches at the authored 40 units.)
- **Records populate asynchronously**, and a dump taken too early reads as a scene with no casters. It
  is not — 525 casters exist once synced. See the correction in `completed.md`.
- **Mover part textures resolve after `RecordSync` has stamped `def = INDEX_NONE`**, which forces
  `castType = 0`. The pawn is the one thing this stream most needs to see, and it is the least
  reliable caster in the scene.

**Mitigated, not fixed**, by `__caster` — a solid box placed through the real record path
([`completed.md`](completed.md)). That covers geometric verification for P1–P2. The mover-texture
timing is a genuine defect but belongs to whatever owns `RecordSync`'s resolve ordering, not here;
P3 needs the real pawn and will have to confront it.

## I15 — `document.hidden` stops the render loop, and it is silent {#i15}

The tab under automation is backgrounded, so `requestAnimationFrame` never fires and **nothing
re-renders after a record write**. Screenshots then show a stale framebuffer that looks like a
correct render of the old state — which is how an early measurement in this session produced a
pixel-difference of exactly 0 between "caster casting" and "caster disabled" and briefly read as
"the caster does nothing".

**Workaround:** one synchronous `__viewport.tick()` before capture. **Not more than one** — a loop of
twelve froze the renderer hard enough to time out CDP.

**Prefer the self-tests to pixels.** `__gather()` runs its own draws and reports counts, so it is
immune to this entirely; it is what produced P0's discrimination table after pixel-sampling failed.


## I16 — P0b's definition-retirement item is a PLAN ERROR {#i16}

> *"Retire `layer`/`seed`/`rotation` from `definition_data` to `u16 reserved`. Acceptance: nothing
> reads the retired lanes."*

**The acceptance is false as written.** `silhouetteHit` reads the definition's `seed` lane on every
sampled texel:

```glsl
float ppu = float((d.z >> 14) & 0xffu) / 8.0;   // the def SEED lane: atlas px per unit x 8
```

That is not a leftover — `RecordSync.defFields` deliberately stores `pxPerUnit × 8` there
(*"the silhouette sampler needs it, and hardcoding 8 was only ever true at the 128-px lod"*).
Retiring it would put every silhouette back on a hardcoded scale and break sampling at any lod but
one. `rotation` is likewise live in `debugDefinition`.

**I wrote this item myself**, generalising "the prim's BLUE frees up" into "the definition's does
too". The prim's BLUE frees because `definition_index` moved in and `layer` was write-only; none of
that reasoning transfers to a record that has no `definition_index` and a live `seed`.

**Not built, and not quietly dropped.** The three lanes it named have different fates:

| lane | in `definition_data` | why |
|---|---|---|
| `seed` | **keep** | `silhouetteHit`'s `pxPerUnit × 8` |
| `rotation` | **keep** | read by `debugDefinition`; harmless |
| `layer` | could go | but it buys nothing — the def's BLUE has no pressure on it |

The item existed to free bits. The definition record has no bit pressure, so there is nothing to buy.
**Recommend closing it as unnecessary** rather than re-scoping — but that is a plan change, so it is
recorded here rather than decided silently.


## I17 — Shadow-TEXEL COUNTS are not reproducible; only `occlusionDiffering` is {#i17}

**Rewritten — the original filing was wrong**, and wrong in a way worth keeping visible.

I first recorded this as "small caster elevations change nothing, large ones change a lot", having
seen `0/4/8/16` produce counts byte-identical to all-zero. Re-running the same inputs A/B/A destroyed
that reading:

| run | caster elevations | shadow texels | corridorWalk | occlusionDiffering |
|---|---|---|---|---|
| 1 | all 0 | 5854 | 5419 | **0** |
| 2 | 0/4/8/16 | 4254 | 3702 | **0** |
| 3 | **all 0** | **5854** | 5419 | **0** |
| 4 | all 4 | 4254 | 3702 | **0** |
| 5 | all 8 | 5090 | 4646 | **0** |
| 6 | all 16 | 5606 | 5163 | **0** |
| 7 | **all 0** | **5131** | 4906 | **0** |

**Rows 1, 3 and 7 are the same input.** They give 5854, 5854 and 5131. The metric varies by ~12% on
identical records, so it was never measuring what I read into it.

**Cause:** the shadow buffer is **ping-ponged and incremental** — the `incumbent` tier is precisely
previously-computed shadows surviving into the next pass. `nonZeroBefore` therefore reports
accumulated state, not a pure function of the current records.

### What this invalidates

Every texel-count delta quoted earlier in this stream is **noise**, not signal:

- P0b's A/B — "4242 → 4253, +0.26%, tracks caster count". It does not track anything; ~12% is the
  noise floor. The conclusion (the relayout changed no behaviour) still stands, but on the *other*
  evidence: `roundTrip.exact`, the six lane assertions, `occlusionDiffering = 0`, and `__elev`
  decoding a fixture's declared `block` and `cardH`.
- P1's ground-unchanged check — "+1 texel of 4254". Also noise. That claim does not need measuring
  at all: at `cElev = 0` the new expressions are `hBot = 0, hTop = subH`, which is the old code
  character-for-character.

### What survives

**`occlusionDiffering` is 0 in every row above, and in every run this session** — 131,072 slot
comparisons each. It is computed fresh within a single pass by comparing the corridor walk against an
exhaustive brute-force reference, so it does not inherit buffer state. **It is the metric to quote.**

**Rule for the rest of this stream: never quote a shadow-texel count as evidence.** Use
`occlusionDiffering`, `roundTrip`, the lane assertions, and `__elev` decodes against known fixtures.
