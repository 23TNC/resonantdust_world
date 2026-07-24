# Issues — world geometry

_Bugs, gotchas, post-mortems, and the P1 audit table. Chronological._

---

## I-1 · CORRECTED — the elevation already matches; only the north offset differs {#i1}
**2026-07-23 — original claim RETRACTED (P1 audit, see I-5).** `shadowCover` places the caster card top at:

```
Yt = A.y − 0.5·H·cos(65)   // north offset  = 0.5·H·cos65
Zt =        H·sin(65)      // elevation     =     H·sin65
```

**The elevation is already correct.** For a sprite drawn `H` tall, the ratified height is `sin65·H` — which
is exactly `Zt = H·sin65`. Elevation-per-drawn-offset is `sin65` in BOTH the card and the ratified model.

The ONLY difference is the **north offset**: the card uses `0.5·H·cos65`; a strictly parallel-to-view
billboard would use `H·cos65` (a `0.5` factor). And that `0.5` may be **deliberate** — the shadow-design
"wedge" uses a `±depth` half-thickness, which is a plausible source of the half. So this is not obviously
a bug; it's a design question (wedge half-depth vs strict parallel-to-view) — see [forks F2](forks.md#f2).

**The original "`2·tan65` vs `sin65`, ~4.7× apart" was WRONG** — it compared elevation÷*north-offset*
(`H·sin65 / 0.5·H·cos65 = 2·tan65`) against elevation÷*drawn-offset* (`sin65`). Different ratios; not a
discrepancy. The I-3 misread, repeated and propagated into this stream.

## I-2 · CORRECTED — `SHADOW_LIFT` is sprite-padding, not a card symptom {#i2}
**2026-07-23 — original claim softened (P1 audit).** The shadow projects its base from `A.y` = the prim's
stored base-centre = `prim.y + prim.height` (the draw-box bottom). The sprite's *visible* base (trunk
bottom) sits ~3 units NORTH of that because sprites carry transparent padding below the content (the `#2`
finding). So `SHADOW_LIFT = 3` seats the shadow on the *visible* base — it pays for **sprite padding**,
which is real regardless of the projection math. It will NOT go to 0 under a corrected card; the earlier
"it's a symptom of a misplaced base, expect ~0" was wrong. (A cleaner fix is to project from the sprite's
actual content-bottom, but that's a def-anchor question, not this stream's.)

## I-3 · Post-mortem: trusting code over geometry {#i3}
**2026-07-23.** While planning prim shadows I quoted the card's `2·tan65` as if it were *the* physical
elevation, then invented a follow-on worry about receivers and casters needing to "agree." Both were
wrong: the card is a tuned fiction (not physics), and the re-projection samples the actual ground shadow
at `G`, so a caster's internal fiction never has to match a receiver's height. Root cause: anchoring on
legacy code instead of the geometry. **This is precisely why the model needs writing down** — with no
canonical statement, the code becomes the de-facto spec.

## I-4 · P1 audit table (done)
**2026-07-23 — audit result.** One row per geometric assumption:

| Assumption | Where | Model it encodes | Matches ratified? |
|---|---|---|---|
| Caster top **elevation** `Zt = H·sin65` | `shadowGather.ts` `shadowCover` | `sin65·H` (drawn height H) | **YES** — already the ratified `sin65·Δ` |
| Caster top **north offset** `0.5·H·cos65` | `shadowCover` | half of parallel-to-view `H·cos65` | **partial** — the `0.5` is the open question ([F2](forks.md#f2)); may be the wedge `±depth` half |
| `casterCover` inversion tilt `65°` | `casterCover` | 65° card, consistent with `shadowCover` | consistent internally |
| `buildCasters` bucket tilt `0.5·cos65·h` | `shadowGather.ts` `buildCasters` (CPU) | same `0.5·cos65` as the card | consistent with the card (would move with F2) |
| `SHADOW_LIFT = 3` | `shadowGather.ts` | sprite-padding seat, NOT card math (I-2) | orthogonal — keep |
| `LIGHT_Z = 40·UNIT` | `shadowGather.ts` | debug light height (units) | fine — a rig value |
| receiver elevation `z = 0x80\|row` | `Viewport.ts` zdepth resolve (`#3`) | tile-row depth, elevation derived downstream | ratified-ready (shadows-on-prims) |

**Net:** the vertical model is ALREADY consistent (elevation `sin65`) between the caster and the ratified
model. The single real open item is the north-offset `0.5` factor (F2). The stream's original premise —
a large ~4.7× misalignment requiring a full re-projection — was a misread (I-1, I-5).

## I-5 · Post-mortem: I mis-documented the misalignment (I-3, again) {#i5}
**2026-07-23.** Writing this stream I stated a `2·tan65` vs `sin65` (~4.7×) elevation gap and built the
whole premise (P2 conform, SHADOW_LIFT-is-a-symptom, shadows-on-prims-must-wait) on it. The P1 audit —
the phase whose *job* is to check assumptions — found the elevation already matches; only the north-offset
`0.5` differs, and even that may be deliberate. Same root as [I-3](#i3): reasoning about the shadow math
from memory instead of reading it. The audit caught it before any code changed. Lesson reinforced:
**read the code in the audit before planning a conform from it.**
