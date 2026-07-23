# Todo — emitter soft shadows (execution order)

_Gated on the corridor↔brute identity diff (`__corridor(false)` + `__gather.debugReadShadow()`,
**0 mismatches**) + visual verification (penumbra widens with emitter). Items move to
[`completed.md`](completed.md) when done **and** verified. Model in [`README.md`](README.md); the
approach fork is [`forks.md`](forks.md)._

## P0 · Shadow coverage u8 → u9 (512 levels) — the format headroom

- [ ] `docs/VARIABLES.md`: shadow-cold slot = **u9** (low8 in-channel + high bit in A[16+i]).
- [ ] Gather pack + overlay decode to u9 (`value = low8 | high<<8`, `/511`). Static accumulators
      (no dynamic write-subscript). The A high-bit plane is bits 16–29.
- [ ] VERIFY: corridor↔brute **0 mismatches** (format is identity-preserving); nonzero deterministic
      (count ANY channel — a u9 value like 256 has low8=0). Zoom round-trip holds.

## P1 · Emitter penumbra math (approach B — silhouette multi-tap)

- [ ] `shadowCover`/`casterCover`: compute `w_world = emitter·(t·Zt)/(Lz − t·Zt)` from the already-
      inverted `t`; convert to atlas texels (÷ card width × frame px per unit).
- [ ] Sample surface B with a small kernel (start 4–9 taps) offset by the penumbra radius in `(s,t)`;
      average → soft coverage. Membership (point-in-quad) still gates so the kernel stays in-range.
- [ ] `emitter_radius = 0` (or below a floor) ⇒ point light ⇒ 1-tap (today's behaviour) — no
      regression for hard shadows.
- [ ] Keep it corridor-identity-safe: the kernel is deterministic per (P, caster, light), so
      corridor↔brute stays bit-identical.
- [ ] VERIFY: **0 mismatches**; visually — grow a light's `emitter` and the shadow edge softens +
      widens toward the tip (sharp at the base); hard shadow at emitter 0.

## P2 · Perf + close

- [ ] Measure the multi-tap cost (taps × per-caster) during orbit vs the 1-tap baseline; tune the
      tap count / kernel. Confirm display-cap holds (or record the real cost).
- [ ] Publish/update the shadow-projection sandbox artifact if it helps tune the width model.
