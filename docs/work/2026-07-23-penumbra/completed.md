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
