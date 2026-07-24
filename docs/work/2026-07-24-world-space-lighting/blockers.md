# Blockers — world-space lighting

_Things that stop progress and need input/resolution. Chronological._

---

## B-1 · P2 gated on the N–S factor confirmation (+ browser access) {#b1}
**2026-07-24 — OPEN, awaiting user.** P0 (the model) + P1 (elliptical falloff behind `__worldlight`, factor
`__nsfactor`) are committed. **P2 (world-space light DIRECTION for the normal `N·L`) is deliberately gated**
— not a manufactured pause:

1. **Same factor.** P2 reuses the exact N–S un-foreshorten factor (`cos65` vs `sin65`, [forks F2](forks.md#f2)).
   Wiring the direction into the relief before the factor is confirmed just compounds an unverified value —
   the discipline this stream's P0 was written to enforce ([issues.md#i1](issues.md#i1); the world-geometry
   `2·tan65` day). The factor is settled by the user's eye/simulation via the P1 oval + `__nsfactor`.
2. **Cross-stream.** P2 changes the relief `N·L`, whose *normal* operand is owned by the separate, user-driven
   [`normal-tilt`](../2026-07-24-normal-tilt/README.md) stream. The two must agree on the world frame — a
   coordination call, not a solo edit.
3. **No self-verify.** The Chrome extension disconnected mid-session, so I can't A/B the oval or P2 myself;
   verification is currently the user's.

**Unblocks when:** the user confirms/tunes the N–S factor (oval reads right, identical across zooms) and says
whether to wire P2 now (unverified, same toggle) or after `normal-tilt` lands its world-frame normals. Then
P2/P3 are mechanical: thread the world direction + the billboard elevation `Pz=zElev` into `LIGHT_FRAG`/the
blit, and the standing-prim front/back darkening falls out.
</content>
