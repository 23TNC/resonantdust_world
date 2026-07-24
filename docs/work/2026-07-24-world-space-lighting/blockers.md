# Blockers — world-space lighting

_Things that stop progress and need input/resolution. Chronological._

---

## B-1 · P2 gated — the FACTOR is resolved; the cross-stream coordination isn't {#b1}
**2026-07-24 — UPDATED. The factor risk is CLOSED; P2 remains gated on `normal-tilt` + a look.**

- **Factor (`cos65` vs `sin65`) — RESOLVED by derivation, not eyeball.** Working in true 3D, the
  light→point squared distance simplifies to `dx² + (dy/cos65)² + Lz²` — the `Z·sin65` cross-terms **cancel
  exactly** ([`model.md`](model.md)). So the N–S factor is `1/cos65`, full stop; `sin65` is the *height*
  coefficient (a different projection), never a competitor. P1 already ships this. The only remaining
  eyeball is a DIFFERENT question — *is the world's art actually 65°?* — answered live by `__tilt(deg)`,
  and it does NOT block P2 (P2 uses the same factor whatever the angle).
- **STILL gating P2 — cross-stream.** P2 changes the relief `N·L`, whose *normal* operand is owned by the
  separate, user-driven [`normal-tilt`](../2026-07-24-normal-tilt/README.md) stream. A world-space light
  direction only reads right once the normal is in the **same** world frame. Wiring P2 before that just
  moves the mismatch, not fixes it — a coordination call.
- **STILL gating verification.** The Chrome extension disconnected mid-session, so I can't A/B P2 myself.

**Unblocks when:** `normal-tilt` has its world-frame normals landed (or the user says to wire P2's direction
anyway, ahead of it). Then P2/P3 are mechanical: thread the world direction + the billboard `Pz=zElev`
(N–S delta off the **base** y) into `LIGHT_FRAG`/the blit; the standing-prim front/back darkening falls out.
</content>
