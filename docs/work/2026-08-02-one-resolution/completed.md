# Completed — one resolution

## 2026-08-02 · P0 — the concepts pinned, the law enforced

**The inventory** ([I1](issues.md#i1)): every consumer of both "lod"s named — the atlas
ladder's ten files vs the slot-grid level's nine, with the two stowaways flagged
(`ZOOM_MIN/MAX` + `TexMap` live in `lod.ts` but belong to the survivor side).
BEFORE numbers: zoom 0.25 packs `{32: 5}`, zoom 1 packs `{128: 5, 32: 5}` — every stem
duplicated (the FLOOR_LOD preview never evicts), 1 pool page. Captures in the session
record.

**The audit** ([I2](issues.md#i2)): live pool — ZERO violators; disk — one real one,
`brick/wall/normal.l.0.png` at 320×320 beside 512 siblings (its `meta.json` already
declared 512; the hand-painted normal never followed the re-master, and a mixed-size
co-pack lands the quadrant misregistered SILENTLY because the pool only warned).
Repaired by LANCZOS resample 320→512 (a resample, not a regeneration — the lock
convention is about laigter re-runs). The 320/640 `sprite`/`diffuse` files are
unserved pipeline intermediates, recorded not repaired.

**The law** ([F3](forks.md#f3)): SQUARE POW2 ≥ 16, chosen over free-size with pros/cons
recorded (every addressing invariant assumes it; ES 3.0 NPOT support only buys margin
bytes). Sign-off: the user armed the stream after reading F3's recommendation; the
fork stays cheap to flip if their verdict differs. Enforcement has TWO faces in the
pool, both REFUSE + `console.error`: non-pow2-square frames, and whole-blit co-pack
sources that are not exactly `quadN²` (the brick/wall class — subframe draws carry
their own source rect and stay exempt). Verified live: brick/wall now co-packs
`{512 → 1024²}` cleanly with the resampled normal; the old 320 would have been
refused; the fixture boots with no `REFUSED` in the console.
