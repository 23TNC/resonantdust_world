# Issues — world geometry

_Bugs, gotchas, post-mortems, and the P1 audit table. Chronological._

---

## I-1 · Two vertical models in the codebase {#i1}
**2026-07-23 — the reason this stream exists.** `shadowCover` places a caster card **leaning back**:

```
Yt = A.y − 0.5·H·cos(65)   // top nudged north
Zt = H·sin(65)             // top elevation
```

Elevation ÷ screen-offset = `2·tan65 ≈ 4.3`. The ratified model gives `sin65 ≈ 0.91` — **~4.7× apart**.
So a screen-offset means one thing to the shadow projection and another to the confirmed geometry. Both
"work" in isolation because the shadow was tuned around its own fiction; they cannot both be our world.

## I-2 · `SHADOW_LIFT` is a compensating fudge {#i2}
**2026-07-23.** The 3-unit lift was measured by eye to seat shadow bases on sprite bases. Under a correct
model that seating should fall out **by construction** — so the lift is (at least partly) paying for
I-1's misplaced base. Treat it as a symptom: re-measure after P2, expect it near 0 ([forks F3](forks.md#f3)).

## I-3 · Post-mortem: trusting code over geometry {#i3}
**2026-07-23.** While planning prim shadows I quoted the card's `2·tan65` as if it were *the* physical
elevation, then invented a follow-on worry about receivers and casters needing to "agree." Both were
wrong: the card is a tuned fiction (not physics), and the re-projection samples the actual ground shadow
at `G`, so a caster's internal fiction never has to match a receiver's height. Root cause: anchoring on
legacy code instead of the geometry. **This is precisely why the model needs writing down** — with no
canonical statement, the code becomes the de-facto spec.

## I-4 · P1 audit table (to fill)
**2026-07-23 — placeholder for the P1 deliverable.** One row per geometric assumption:

| Assumption | Where | Model it encodes | Matches ratified? |
|---|---|---|---|
| _(P1 fills this)_ | | | |
