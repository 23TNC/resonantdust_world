# Completed — render performance

_The verification log: dated entries saying what landed and HOW it was checked. Append-only;
authoritative for what is done and why we believe it. Item text lives in `todo.md`._

Nothing yet — the stream was opened 2026-08-02.

## 2026-08-02 — P1 partial + P3 wiring; the preview tier is built but INEFFECTIVE

**Landed in `TextureResolver`:**

- **Parallel preview + master kick.** `resolve()` fires `PREVIEW_PX` (32) and `entry.maxSize`
  together; both kicks are idempotent. Verified live — a manual `resolve()` on
  `biome-thing/default/conifer/e` queued `@32` and `@256` in the same call, and the served manifest
  then reported `'lods': [32, 256]` for that stem where every other stem still reads `[]`. So the
  edge really derived and cached the requested preview.
- **`packedSize`, the never-downgrade guard.** Checked on entry AND re-checked after the bytes land,
  because the master can arrive while a preview is in flight. Cleared at every eviction site
  (`setSpriteScale`, `setSubframe`, `setLinkedPad`, `onManifestChange`) — without that a re-mastered
  stem is blocked from re-packing at its own size, forever.
- **[I2](issues.md#i2) fixed: `emit()` on failure too.** `if (ok) emit()` left a stem whose bytes had
  arrived sitting on geo for the whole session with no re-bake ever scheduled.
- **The bbox is derived from the MASTER only.** Guarded by `size >= entry.maxSize`. Deriving it from
  whichever size decoded first is exactly
  [subframe-ingest I7](../2026-08-02-subframe-ingest/issues.md#i7) — the same art measuring
  `fy 0.125` one session and `0.109` the next.

**The GEO resting state is REAL — proven, not assumed.** On a cold load the world drew fully as flat
tinted geometry with `packed: 0`. That is [F1](forks.md#f1)'s claim demonstrated rather than argued.

**But the tier does not help yet — [I8](issues.md#i8).** Cold-cache measurement: `packed: 3`,
**`prev: 0`, `master: 3`**. The master wins every race, because the edge DERIVES a preview from the
master (read, decode, resize, encode, cache) while the master is a direct file read — the preview's
critical path strictly contains the master's. Previews must be **generated as assets**, which is what
the user's original instruction said and what I misread `textures.rs`'s "no offline pyramid" as
solving.

**Two measurement traps hit and worth recording:**

1. **A backgrounded tab never runs rAF**, so a CDP-driven "cold load" bakes nothing, resolves
   nothing and fetches nothing — I measured "12 s and no textures" that was purely the harness.
   `vp.tick(16)` in a loop is the fix, as the subframe-ingest fixture note already warned.
2. **`__viewport` does not exist until ~6.2 s** on a cold load, which is itself unexplained and
   larger than anything in the texture path. Worth a look before optimising fetches further.
