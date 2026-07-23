# Todo — presence + buckets into the data texture (execution order)

_Each phase verifiable on `/overlayRT shadow-cold` + the corridor↔brute identity diff
(`__corridor(false)` + `__gather.debugReadShadow()`, must stay **0 mismatches**) + the zoom
round-trip (silhouettes persist, nonzero deterministic) + a **pan-across-region-boundary** check
(eviction correctness). Items move to [`completed.md`](completed.md) when done **and** verified.
Model + fold in [`README.md`](README.md); decisions in [`forks.md`](forks.md)._

## P0 · Authoritative layouts + the persistence principle

- [ ] `docs/VARIABLES.md`: the two new sets (presence-light, caster-buckets — u16 id + 7 slots), the
      **region-torus fold** (zone-strip packing), and a short **DATA = persistent** rule (with the
      dirty-stays-out note). Confirm the base assignment (presence-light = set 3, buckets = set 4).

## P1 · Fold presence + buckets into the data texture (region-torus)

- [ ] `coldShadowData`: add the two sets (self-addressing writes: u16 id in R high, 7 slots). Retire
      the separate `presenceTex` / `casterTex` (+ their resize logic).
- [ ] `buildPresence` / `buildCasters`: write into the data mirror at the **fold** slot
      (`SET_BASE + fold(wc,wr)`) instead of the window-relative `pmod(cols)` slot; mark dirty →
      the scatter flushes them as presence/bucket commands (SAME pass).
- [ ] Gather + corridor: read presence/buckets via `fetchLin(uData, SET_BASE + fold(wc,wr))` (the
      corridor walk moves from slot-space to the fixed region-torus). Overlay reads presence the
      same way. Drop the `uPresence` / `uCaster` bindings.
- [ ] `fold()` shared TS ↔ GLSL (one definition, both sides identical).
- [ ] VERIFY: identity 0 mismatches; zoom round-trip {lod0, lod5, lod6}; nonzero deterministic;
      overlay renders presence-coloured shadows correctly at zoom 0.5 / 1.

## P2 · Eviction + allocation

- [ ] **Region-window eviction** for presence/buckets: materialise only the anchor-centred 16×16
      zone block; clear a slot when its owner-zone leaves the block (owner-change tracking at the
      fixed region modulus — mirror `shadow_dirty`'s per-slot owner logic).
- [ ] **Prim/light free-list**: freed u16 ids → a stack; alloc pops it (falls back to `primNext++`).
      Free a prim/light when it leaves `standing` (its zone evicted / object destroyed) — deep layer.
- [ ] Confirm the two eviction layers don't interfere (a prim freed only after its region tiles are
      gone — the reach gap; [`README`](README.md#eviction)).
- [ ] VERIFY: pan a full region in one axis — presence/buckets evict + rematerialise correctly, no
      stale shadows, identity holds throughout; a back-and-forth jiggle near a boundary does NOT
      thrash (16-zone hysteresis).

## P3 · Perf + close

- [ ] Measure commands/frame + bytes/frame during streaming / orbit / region-crossing vs the
      unified-data baseline; confirm ONE scatter pass (no second FBO).
- [ ] Decide [`forks.md#f1`](forks.md#f1) (tie eviction to the subscription model) — build now or
      leave as the recorded follow-up.
