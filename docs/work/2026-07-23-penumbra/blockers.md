# Blockers — emitter soft shadows

_Open blockers; remove the row when cleared._

---

## P1 · penumbra APPROACH decision (F1) — awaiting user

The penumbra math has a real quality/cost fork ([`forks.md#f1`](forks.md#f1)): silhouette multi-tap
(real penumbra, N extra B samples/caster — RECOMMENDED) vs quad-band (cheap, but the silhouette
hides the quad edge → weak). P0 (the u9 format) is done + verified; P1 execution waits for the
user's call on the approach (and the tap-count/width tuning that follows). Clear this on "go with
multi-tap" (or the chosen approach).
