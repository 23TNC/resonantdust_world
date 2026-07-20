# Todo — shadow-cast (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. See
[`README.md`](README.md) for the experiment, [`forks.md`](forks.md) for decisions, [`issues.md`](issues.md)
for the gotchas._

---

## S0 · 5 cold lights + a caster source — 2026-07-20

- [ ] A tiny light type `{ x, y, z, radius }` (world px) + an array of **5** on the viewport, seeded in the
      zone at **(100, 50)** via a `/shadowcast` debug command (+ `?shadowcast` URL). Each light owns **bit
      `k`** (0–4).
- [ ] A caster source: re-add `standingPrims()` (`zIndex ≥ 1`) to `SquareCache` (the nuke removed it), or
      plant a few test caster prims in the zone if simpler ([F5](forks.md#f5)). Per-light **in-radius cull**.

## S1 · RTs + display — 2026-07-20

- [ ] **`shadow-a` / `shadow-b`** — two unorm RGBA8 bitfield RTs (screen-space, viewport-sized, `nearest`,
      **A=1**), ping-ponged ([F2](forks.md#f2)). Bits 0–4 live in the **RED** byte ([F1](forks.md#f1)).
- [ ] **`mask`** — a small coverage RT (the dirty light's shadow union; a mini `shadow-hot`).
- [ ] **Display**: a screen-space pass decoding the current bitfield → **5 distinct colours** (one per bit,
      additive overlap), float-mod (ES 1.00) ([F6](forks.md#f6)). Reuse the `bitfield-rt` decode shape.

## S2 · Cast pass → `mask` — 2026-07-20

- [ ] For the **dirty** light `k`, project each **in-radius** prim's **billboard-quad shadow** from the
      light to the ground, map world→screen, and rasterise its coverage into `mask` (**`max` blend**, so
      overlapping prims union at 1). Simplest shape — no silhouette/UV ([F5](forks.md#f5), [shadows D-3](../shadows/deviations.md#d-3)).
- [ ] Clear `mask` before casting; only the dirty light's prims are drawn ("send only dirty lights + their
      prims").

## S3 · Combine pass → dest (carry-forward + re-cast the dirty bit) — 2026-07-20

- [ ] A full-screen pass reads **`src` (current bitfield) + `mask`**, writes **`dest`** (the other buffer):
      `dest = (src with bit k cleared) | (mask > 0.5 ? bit k : 0)`. The other 4 bits are **copied from
      `src` unchanged** ([I-2](issues.md#i-2)). Float-mod bit ops; A=1; no blend. **Source ≠ destination**
      ([I-1](issues.md#i-1)).
- [ ] Swap `src`↔`dest`; display `dest`. Between updates (no dirty light), just keep displaying the current
      buffer — no ping-pong on idle frames.

## S4 · Move one light per second — 2026-07-20

- [ ] Every ~1s, move **one** light (round-robin over the 5) to a new spot in the zone → mark it dirty →
      run S2+S3 for that light only. (Timing from the tick's elapsed ms; no `Date.now` needed.)

## S5 · Verify — 2026-07-20

- [ ] `?focus=100,50&shadowcast`: **5 shadows in 5 distinct colours** fanning from the 5 lights, overlaps
      blending. Casters within a light's radius shadow; outside don't.
- [ ] **Incremental proof:** when one light moves, **only its colour** shifts — the other 4 colours stay
      pixel-stable (they were carried forward, not re-cast). This is the core thing to see.
- [ ] Confirm a light moving **off** a spot **clears** its bit there (the clear-bit-k before re-cast,
      [I-3](issues.md#i-3)) — no ghost shadow left behind.
