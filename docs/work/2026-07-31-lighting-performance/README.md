# Lighting performance — steady-state gating — 2026-07-31

_Component: [`client/webgl`](../../components/client/webgl/). Successor to
[`2026-07-31-lighting-visual`](../2026-07-31-lighting-visual/README.md) (visuals signed
off first — the user's sequencing directive). The correctness stream's verdict named the
lever: the chain is UNGATED — every frame re-runs gather → receivers → slots → sum over
the whole window whether anything changed or not, so a STATIC scene costs the same as a
moving one. The design already stores everything gating needs (per-light slots, stored
caster identity, bit-exact deltas); this stream wires the "don't do it again" half._

## Baseline (measured at stream open, the visual stream's final state)

`__framecost` at the standing fixture, reach-16 lights, whole chain per frame:

| lights | moving (ms) | static (ms) |
|---|---|---|
| 1 | 2.34 | — |
| 4 | 4.82 | — |
| 8 | 8.52 | 8.00 |
| 16 | 9.76–10.24 | 9.55 |

Static ≈ moving is the finding: nothing is gated. The slot pass alone rasterises
16.7 M fragments/frame (8 slots × the window). The stripped unlit floor is **0.104 ms**
— the idle target's order of magnitude. Differential exactness is already proven
(`__lightexact` bit-identical add/remove via the delta path), so gating cannot drift the
image: the steady-state path deposits/withdraws EXACTLY what the full pass writes.

## The levers (the correctness stream's verdict, in build order)

1. **Skip-when-unchanged**: if no prim record changed, no emitter signature changed, and
   the window did not move, issue NO lighting passes — the summed map and shadow buffer
   are already right. The reconciler already computes change sets; the gate reads them.
2. **Per-light scissored updates**: a change that touches one light (a mover crossing a
   pool, a flicker) re-slots ONLY that light's rectangle via the existing
   withdraw/write/deposit delta path — never the whole window.
3. **Dirty-rect uploads**: prim/light/presence texture uploads shrink to the changed
   rows instead of full-texture re-upload.
4. **Pan increments**: a window move re-lights only the entering strip (the toroidal
   maps already keep the rest addressable).

## Acceptance model

Numbers, not vibes: `__framecost` tables beside the baseline above, plus `__lightexact`
bit-identity after every phase — gating must never change what is drawn, only whether it
is recomputed. The idle target: a static 16-light scene within ~2× of the unlit floor.
