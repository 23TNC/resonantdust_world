# Issues — world-space lighting

_Bugs, gotchas, and cautions. Chronological._

---

## I-1 · The trig is the whole risk — derive + simulation-check before code {#i1}
**2026-07-24.** The value of this stream is one correct world-3D light vector; its only real risk is getting
that vector wrong. The exact N–S factor + how the light height enters is the same sign/factor class that
produced the retracted `2·tan65` misread on world-geometry ([issues I-1/I-5](../2026-07-23-world-geometry/issues.md#i1)),
which cost ~a day. Discipline (P0): derive from the ratified `z = sin65·Δ` model, write it down, and confirm
against the user's simulation BEFORE any shader change. Everything downstream is a mechanical swap once the
vector is right.

## I-2 · Audit where screen-space lighting assumptions are baked in {#i2}
**2026-07-24.** The flat-screen assumption isn't only in the falloff. Before/while implementing, list every
place lighting treats a 2D screen quantity as a world one: the falloff `length(Lxy − P)` and the aggregate
direction `toL/dist` in `LIGHT_FRAG`; the relief `dot(N.xy, ldir)` in the blit; and confirm the SHADOW
projection (`shadowCover`) is out of scope (it runs its own tilt model and shouldn't be double-corrected).
Missing one leaves a half-corrected pass that looks worse than either pure version.

## I-3 · Verify at multiple zooms — foreshorten is a WORLD property, not a screen one {#i3}
**2026-07-24.** The N–S foreshorten is a fixed function of the `65°` world tilt, in world units — it must be
**identical at every zoom**. If the corrected oval changes shape as you zoom, the factor leaked a
screen/pixel quantity (the exact bug that reverted shadows-on-prims twice). Bake the check into P1/P2.
</content>
