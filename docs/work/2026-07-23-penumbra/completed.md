# Completed — emitter soft shadows

_Done **and** verified (identity diff + visual penumbra). Items move here from [`todo.md`](todo.md).
Append-only history; authoritative for what's done._

---

## P0 · Shadow coverage u8 → u9 (512 levels) — 2026-07-23 ✓

- `docs/VARIABLES.md`: shadow-cold slot = u9 — low8 in-channel (`i>>2`, `(i&3)·8`) + high bit in
  A[16+i] (A bits 16–29 = the 14 high bits; 30–31 reserved). `value = low8 | high<<8`, 0..511.
- Gather packs the high bit into A separately (static accumulators, no dynamic write-subscript);
  overlay reconstructs + `/511`.
- **Verified**: corridor↔brute **0 mismatches** (format identity-preserving); nonzero (any-channel)
  deterministic 11,413; zoom holds; console clean (the two errors seen were STALE from the earlier
  PRESENCE_HI_BASE compile miss — a post-clear read showed none). u9 = the coverage resolution P1's
  penumbra gradients need.

## P1 · Emitter penumbra + umbra (multi-tap) — 2026-07-23 ✓

- `casterCover` gains an `emitter` param (extracted from the light record A word, units); a 3×3
  `sampleSoft` kernel averages surface B over 9 taps at radius `kr` px (out-of-frame taps count 0 →
  the penumbra fades at the sprite-frame edge). `kr < 0.5` → single sharp tap (point light / near
  base). Width: `w = emitter·(t·Zt)/(Lz − t·Zt)` from the already-inverted `t`, `→ px` via `ppu`,
  `÷ k` (ground→card), `× PENUMBRA_SCALE`, clamped to 12 px.
- **Umbra emerges** (F4, physical): `w→0` at the base keeps coverage 1.0 (sharp dark umbra); `w`
  grows toward the tip so the kernel washes out thin features → peak drops → umbra recedes, lighter.
  Base-dark → tip-light gradient with NO separate term.
- **Verified**: corridor↔brute **0 mismatches** (multi-tap is deterministic per P/caster/light);
  **10,255 partial-coverage texels** (10<v<245) where before coverage was near-binary — the gradient
  is real; visually the shadows show dark cores at the tree bases fading to soft translucent tips;
  hard shadow at `emitter 0` by construction; console clean.

## P2 · Perf + close — 2026-07-23 ✓

- **9-tap holds the 121 display cap** at static AND orbit (moving light, full recompute every frame).
  The multi-tap (9 × the per-caster B sample, corridor-bounded) is affordable in this 1-light scene;
  revisit the tap count if many overlapping lights land. Kept 9-tap for quality.
- Debug `LIGHT_EMITTER = 40 px (10 units)` is a large value to SHOW the effect — production emitter
  comes from content per light (DSL `emitter_radius`). `PENUMBRA_SCALE = 1` (physical); raise for
  softer. Explicit contact-darkening ([`forks.md#f4`](forks.md#f4)) NOT needed — physical falloff
  reads well.

_Stream DELIVERED: emitter-based soft shadows — penumbra (soft edges) + umbra (dark core receding
with distance), on the u9 coverage, verified 0 mismatches + visually._