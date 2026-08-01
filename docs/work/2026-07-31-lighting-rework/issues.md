# Issues — lighting + shader rework

_Problems hit, candidate solutions, which we chose and why. **Open issues only** — solved ones are
removed once their resolution is logged in [`completed.md`](completed.md), because a file that mixes
live problems with settled ones stops being a list of what needs attention.

Removed 2026-07-31 as solved: **I1** (the `l > 4` slot split — fixed by `pairSlot`, P4), **I3** (the
span/subframe bias — implemented and verified, P1), **I9** (P1's render-from-records plan error —
built as its intent instead). Numbering is not reused; git holds the text._

## I2 — Receiver selection breaks before testing coverage, and is per-light

```
for r in presence[7..1]:
  if (shadow[i][ll]) && (shadow[i][ll+1] == r):
    draw_shadow(...); break        // breaks WITHOUT testing r.has(px)
  if r.has(px):
    draw(...); break
```

If the stored receiver for light A is `r7` and the pixel is only covered by `r6`, light A matches at
`r7`, **breaks**, and never reaches `r6`. Light B, with a different stored pair, falls through and
lights `r6` correctly. The same pixel resolves to a different surface per light, and renders as a pixel
missing one light's contribution.

**Fix: test coverage first.** Coverage is geometry — it cannot depend on which light we are
evaluating:

```
for r in presence[7..1]:
  if !r.has(px): continue
  if shadow[i][ll] && shadow[i][ll+1] == r: draw_shadow(light[l], shadow[i][ll], r, px)
  else:                                     draw(light[l], px, r)
  break
```

**And it unlocks a real saving.** Once selection is light-independent it hoists out of the light loop:
`r.has(px)` is a texture sample, so this goes from up to 56 samples per pixel (8 lights × 7 receivers)
to 7 — in the pass that dominates the frame. [`todo.md`](todo.md) P5 carries both halves.

## I4 — The design's stale prose lines, after the layout edits

Three comment lines describe a layout that changed underneath them:

| line | says | actually |
|---|---|---|
| `color.1..3 are the tints…` | three colours | four — `color.4` is the emitted light colour |
| `u8 normal.r/g/b are normals…` | prims carry a facing normal | orphaned; `GREEN` no longer carries normals |
| `ALPHA … overrides like color/seed` | seed lives in `ALPHA` | `seed` moved to `BLUE` |

Cosmetic, but this file becomes `VARIABLES.md` in P7 and these lines would be inherited as truth.
Same for the spellings that will otherwise become field names forever: `recieve`, `sentinal`, `psudo`,
`coorespond`, `roation`.

## I5 — Inherited: the per-pixel pass is the one cost nobody has bounded

**Evidence:** [strip P1](../2026-07-31-lighting-strip/completed.md) — the stripped renderer is
0.045 ms/frame at 1 draw; the old lit renderer was 0.508 ms static / 0.675 ms with a moving light, so
lighting was ~91 % of the frame.

The per-**unit** gather is not the worry: it runs 256 fragments/tile, the same resolution the old
gather ran at, which measured **0.603 ms at one light, reach 16** — and the rework deletes the search
that made the old one expensive.

The per-**pixel** pass is new. It is per screen pixel × 8 lights with a receiver loop inside, and the
design does not bound it. [`todo.md`](todo.md) sequences P3 (lighting, no shadows) before P4 (shadows)
specifically so **cost-per-light is measured before anything is built on top of it** — the opposite of
the old stream, which built the refine first and discovered it cost 9.29 ms of a 10.88 ms pass.

## I6 — Inherited: what the previous system could do, as an acceptance list

Not a problem — a checklist, so "does the new system do X" is answerable rather than argued. From the
strip's reference renders:

1. Point lights with soft radial falloff, authored per content kind
2. Projected silhouette shadows, direction per caster from angle-to-light
3. Shadows onto billboards as well as ground (the climbing shadow)
4. n/s perpendicular caster cards for rotated billboards
5. Movers lit and casting in the same pass as static geometry
6. Per-light N·L against a normal map
7. Emissive (self-lit) pixels
8. Ambient × AO on the omnidirectional term
9. Decay/flicker glow
10. Bilinear shadow upsample

Items 7–9 were **cut with the strip and are not in the rework design**. That is a deliberate loss to
re-decide, not an oversight to fix silently — P7 records it in the A/B.

## I7 — 8 lights per tile is an artefact of the OLD bit budget {#i7}

Eight was never a design ceiling. It was `128 bits / u16` — the number that made caster ids fit the
shadow texel exactly ([strip I1](../2026-07-31-lighting-strip/issues.md#i1)). The rework keeps 8, and
should not keep it *for that reason*, because the reason no longer exists.

In the new model the cap is set by the **slot map**, not by a texel: 8 slots is 8 px per texel and
64 MiB; 16 would be 128 MiB. That is a memory-vs-density trade with a measurable price, which is a
different question from the packing constraint 8 came from.

Two things pull against each other, and neither is resolvable from the desk:

- The design says _"dense AUTHORED point lights, not a sun"_, which argues for more per tile.
- The per-tile cap is on **overlap at one tile**, not on how many lights exist — tile A can hold
  lights 1–8 while tile B holds 9–16 — so 8 overlapping at a single tile may already be generous.

**Revisit at P7, with the cost per light from P3 in hand.** Recorded now so that "8" is a decision
someone made with numbers rather than a constant that survived three rewrites because it looked
familiar.

## I8 — The design's atlas quadrants have BL/BR swapped {#i8}

**Still open — it needs an edit to `docs/intent/2026-07-31-rework.md`, which is the user's file.**
(The other half of this issue, "build the packer", was closed at P1: `TextureResolver` already
co-packs each stem's four maps into one `2N × 2N` frame, verified by sampling.)

Sampled on live data (`biome-thing/default/conifer/e`, lod 0) — four equal quadrants of a 256×256
frame, so `fullframe.span == 2 × frame.span` holds exactly as the design derives:

| map | offset in N |
|---|---|
| albedo | `[0, 0]` TL |
| normal | `[1, 0]` TR |
| **surface** | **`[0, 1]` BL** |
| **layers** | **`[1, 1]` BR** |

The design says *layers* at `(frame.x, frame.y + span)` and *surface* at
`(frame.x + span, frame.y + span)` — the opposite. The live table (`TextureResolver.QUADRANT`) is
authoritative because the G-buffer bake reads it every frame, so a shader built from the doc would
sample the wrong quadrant for two of the four maps.

Those are the four lines prefixed *"I believe that puts…"*, which is an invitation to verify.
**Raised rather than silently edited**, per the stream's acceptance that the intent doc owns its own
content.

## I10 — `gl.finish()` does not sync in this environment; every earlier ms number is low {#i10}

**Found 2026-07-31 while measuring P3's cost-per-light, by disbelieving the result.** The lighting
pass reported **0.003 ms** flat across N = 1, 4, 8, 16. Flat is suspicious; 0.003 ms is impossible.
16 777 216 fragments in 0.003 ms would be **5.6 Tfragment/s**.

Three things were wrong, each hiding the next:

| # | fault | effect |
|---|---|---|
| 1 | **`performance.now()` is coarsened to 0.1 ms** in a backgrounded tab | 30-iteration runs totalled ~0.09 ms — *entirely below the clock*. The "0.003" values were quantisation, not measurement |
| 2 | **`gl.finish()` does not sync** | Chrome runs GL in a separate process behind a command buffer; `finish()` returns before the GPU has done the work |
| 3 | **`readPixels` with a mismatched format silently does not sync either** | `RGBA16F` read as `UNSIGNED_BYTE` raises `INVALID_OPERATION`, returns nothing, and syncs nothing — still 20× low |

Only `readPixels` with the **matching** format (`RGBA16F` → `FLOAT`) forces a true sync, because it
has to hand back real pixels and cannot pretend.

### What the numbers actually are

| sync method | ms per lighting update | implied fragment rate |
|---|---|---|
| `gl.finish()` | 0.002 | 8 Tfrag/s — impossible |
| `readPixels`, wrong format (errors) | 0.014 | 1.2 Tfrag/s — impossible |
| **`readPixels`, correct format** | **~0.30** | **55 Gfrag/s — plausible** |

Confirmed by a scaling test: total time is linear in iteration count (0.7 / 1.5 / 2.7 / 4.6 / 9.2 ms
for 50 / 100 / 200 / 400 / 800), so it is measuring work rather than overhead.

### What this invalidates

**Every ms figure taken before this**, in both streams, is low by roughly 4×. Re-measured with the
corrected harness on the identical unlit scene:

| | reported | corrected |
|---|---|---|
| unlit frame | 0.028 ms | **0.104–0.116 ms** |

The spread also tightened from ±0.016 to ±0.003 — a noisy instrument getting quiet is itself evidence
the fix is real.

**Affected, and NOT retro-edited** (`completed.md` is an append-only log; correcting it in place would
hide that the measurements were ever wrong):

- the strip's P1 lit/unlit comparison (0.508 / 0.675 / 0.045) — the *ratio* is probably close, since
  both sides shared the fault, but the absolutes are low
- the strip's P2 "0.045 → 0.028" improvement
- this stream's P0 harness acceptance, and [D1](deviations.md), which argued about 0.045 vs 0.028
  when **both** were wrong

**`frameCost.ts` is fixed** and its header now carries the measurement, so the next person does not
re-derive it. The lesson generalises past this bug and matches [I7](#i7) exactly: in this environment
the wrong measurement method **does not error, it under-reports** — so a number that looks good is the
thing to distrust first.

## I11 — I measured STATIC lights and reported them as "moving" {#i11}

**The stream's headline acceptance is "moving lights at reach 16, zoom 1, inside 8 ms". I placed 16
lights once, never moved them, and reported 2.31 ms as if it answered that.** It does not.

Why it matters rather than being a wording slip: **static lights are the cheap case, and they are
cheap for a reason this design is built around.** With nothing moving, the incumbent tier answers
66.9 % of pairs — last frame's caster is still the caster, so the corridor walk barely runs. Move the
lights and incumbents invalidate every frame, pushing work onto the walk, which is the expensive tier.
The number I reported is therefore **the best case of the exact mechanism the headline was meant to
stress**.

The honest position: cost-per-light and the frame cost are real measurements of a static scene. The
**headline number does not exist yet**.

## I12 — The refine tests a solid rectangle, not the sprite silhouette {#i12}

`occludes()` and `refineOccluded()` model a caster as a **solid card**: half-width from
`subframe.width`, height from `subframe.height`, occluding anywhere the ray crosses it. A conifer is
therefore a **rectangle** to the shadow system, which is why the shadows on screen are blocky wedges
rather than tree-shaped.

The design is explicit that this is not the intent — *"the fine detail lighting pass will actually
place the section of the casting prim's texture that falls into the slot"* — and the old system did
sample the sprite's alpha (`design/shadows.md`: *"Sample the sprite **alpha** as the shadow mask"*).

So the stored-identity architecture is working: it finds the right caster and re-tests it at 64/tile.
It is re-testing **the wrong shape**. The fix is local — sample the caster's `surface` quadrant at the
crossing point instead of accepting the whole rectangle — and it is the difference between "there is a
shadow here" and "this shadow is that tree".

**Both of these were ticked as complete. They are not, and the stream is reopened.**

## I13 — Four shadow defects, user-reported 2026-07-31, OPEN {#i13}

Reported after the silhouette + motion fixes: *"You goofed on the minimum bbox, you have no
silhouette, the shadows aren't anchored at the base of our billboards… z-ordering is quite wrong."*
Recorded verbatim rather than paraphrased, because I have twice now called this area fixed when it
was not, and my summary is not the trustworthy artefact here.

| # | defect | what I know |
|---|---|---|
| 1 | minimum bbox is wrong | `buildRecords` writes `subX/Y/W/H` from `opaqueBBox` fractions × `span × 16`. Dimensionally plausible, never verified against the drawn sprite |
| 2 | no silhouette | `silhouetteHit` samples `surface.b`, but the shadows still read as blocks — so either the texel maths, the quadrant offset, or the lane is wrong |
| 3 | shadows not anchored at the billboard base | the card is modelled rising from `C.y` with `C` = the prim's base-centre; the on-screen offset says that mapping is off |
| 4 | z-ordering wrong | the blit samples the lightmap by `vWorld = aPosition`, so a billboard's pixels take the light of the ground they are DRAWN over, not the tile they STAND on — the same defect `2026-07-30-pawn-part-placement` I1 records against the old renderer |

**Defect 4 has a known cause and a known fix** (read the light at the prim's base row, which
`zdepth.B` already carries). 1–3 need looking at on screen at zoom, not reasoning about from the
source — which is what I should have done before claiming them.
