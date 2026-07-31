# Issues — lighting + shader rework

_Problems hit, candidate solutions, which we chose and why. I1–I3 are **corrections to the design
document** found while reviewing it; they are recorded here so the fix survives even if the intent doc
is edited around them._

## I1 — The light→slot split drops light 4 and overruns px 1

`docs/intent/2026-07-31-rework.md`, both loops:

```
if l > 4:  i=2; ll=(l-4)*2      // l=5,6,7 -> ll=2,4,6  (px2 slots 0,1 never used)
else:      i=1; ll=l*2          // l=4     -> ll=8      (px1 has slots 0..7)
```

A px holds 8 `u16` slots, so `ll` must land in 0..7. At `l=4` the else-branch produces `ll=8`, one past
the end of px 1, and the if-branch never covers `l=4` at all.

**Fix: `l >= 4`.** Then `l=0..3 → i=1, ll=0,2,4,6` and `l=4..7 → i=2, ll=0,2,4,6`, which fills both px
exactly. Carried into [`todo.md`](todo.md) P4 as an acceptance criterion rather than a note, because
the symptom — one light in eight silently never shadowing — is exactly the kind of thing that reads as
"the shadows look wrong" for a week.

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

## I3 — Off-by-one in the span and subframe encodings

`frame.span` is `u4` (0–15) against `MAX_TEXTURE_SPAN = 16`; `subframe.width`/`height` are `u8` (0–255)
against a stated max of 256.

**Resolved by the user: stored biased — 0 means a span of 1, 15 means 16**, since a span of 0 would be
no frame at all. The same convention applies to subframe width/height. Recorded because it is invisible
in the layout table and a reader who assumes the natural encoding will be exactly one off, in a lane
that silently produces a wrong-sized sprite rather than an error.

`subframe.x`/`y` are genuine 0-based offsets and take **no** bias — the two conventions sit in adjacent
lanes of the same channel, which is precisely why this needs writing down.

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
