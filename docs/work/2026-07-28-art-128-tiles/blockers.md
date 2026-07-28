# Blockers — art pipeline on 128 px tiles

_Only what genuinely needs the user. A decision I can make is a [fork](forks.md), not a blocker._

## B1 — RESOLVED 2026-07-28 by the user {#b1}
_was: the 17 undersized leaves are two problems and both need your call_

**(a)** convert the linked forms to `sprite.l.0.png` + delete the per-cell folders → [F4](forks.md#f4).
**(b)** sprite is an ARCHIVE; remaster scales every derived map to the target square in all cases, warning on upscale → [F6](forks.md#f6). Both landed; the original text is kept below.


The [P4.1 audit](completed.md) found **78 stamped leaves: 61 already right, 17 undersized, 0
oversized.** But "re-master the undersized" is the wrong action for either group.

### (a) `blueprint/wall/1..16` — 16 leaves at 160 px wanting 256

These are the **superseded per-cell split**. The
[design](../../components/dev/textures/design/texture-layout/README.md) decision 7 says a linked
form is one held-whole atlas with **no per-cell `<variant>` folders**, and marks the old shape
⚠️ NOT YET. The kind still carries its atlas source beside them (`1.l.0.diffuse.png`).

Re-mastering would upsize sixteen folders the design says should not exist. The consistent action is
to **delete** them — but `textures/` is **gitignored**, so there is no history to recover from and I
will not delete unversioned art on my own judgement.

- **Recommended:** delete `biome-tile/default/blueprint/wall/{1..16}/` and let the held-whole path
  regenerate from `1.l.0.diffuse.png`. That path is unreachable until `GRID_CATS` moves to
  `biome-tile` ([I12](issues.md#i12)) — the same decision, so resolving this resolves [F4](forks.md#f4).
- **Alternative:** keep them, exclude them from the audit, accept a permanently red count.
- **Not recommended:** re-master them to 256 — it entrenches a shape the design supersedes.

### (b) `smooth/wall` — the real linked atlas, 320 px wanting 512

Genuinely undersized: 4×4 cells at 128 px is 512², and the master is 320² (80 px per cell). The only
leaf where the 128 px move actually demands new pixels.

I can make the number correct by upscaling, and I do not think I should. 320 → 512 is a 1.6×
upscale: a 512 file carrying 320 of real detail. That is precisely the softness
[`square-128`](../2026-07-28-square-128/README.md) exists to remove, and the audit would go green
while the art got worse.

- **Recommended:** re-author or re-generate the wall atlas at 512² (4×4 cells of 128), as the grass
  sheet was regenerated. Content work, not pipeline work.
- **Alternative:** upscale now to unblock the packer and flag it — honest only if recorded, since
  nothing downstream can distinguish an upscaled 512 from a real one.
- **Do nothing:** it packs as 320 and displays soft at 128 px tiles.

**Not blocked by this:** the ground sheets are done (P4.3 — grass 8×8×128, seam 0.99), and P5 is
documentation. Both can proceed.

---

**Not a blocker, but a sequencing note:** `SQUARE = 128` is owned by
[`2026-07-28-square-128`](../2026-07-28-square-128/README.md). This stream is parameterised on the
tile edge rather than gated on it ([P1](todo.md)), so the two can land in either order. If that
stream is abandoned, the art here is authored at 128 and displayed through a 64 px cap — softer, not
broken.
