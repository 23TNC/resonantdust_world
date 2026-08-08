# Plan — surface channels

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the issue inventory in [`issues.md`](issues.md) (I#)._

## P0 — the paper: one place that says what the channels are

- [ ] VARIABLES.md: a **Texture map channels** section — the LEAF layout and the G-buffer
      ATTACHMENT layout side by side (they differ), plus the no-alpha and
      A-is-a-blend-factor laws. Acceptance: `rd docs-check` green.
- [ ] Name each lane's readers in that section, so an unread lane is visible as unread.
      Acceptance: R, G and B each list their consumers or say "none".
- [ ] Delete the three lying comment blocks ([I5](issues.md#i5)) in `_surface_kind`,
      `bin/art`'s header/echoes, and `albedoBlitShader`. Acceptance: `grep -rn "R=height\|R
      = height"` over the repo returns nothing.

## P1 — resolve G (the user's first ask)

- [ ] `depth_ao.py`: teach discovery the FOLDED leaf name ([I1](issues.md#i1)), matching
      `bin/art`'s `_find_diffuse`. Acceptance: `find_diffuse` on a conifer leaf dir returns
      one path per direction/part.
- [ ] `depth_ao.py`: same treatment for the WRITE path (`d.with_name("occlusion.png")` emits
      the flat name). Acceptance: `art ao textures/biome-thing/default/conifer` writes
      `occlusion.e.0.png` in place.
- [ ] Delete `depth_ao.py`'s `--surface` writer, flag and help — the contradictory second
      layout ([F2](forks.md#f2)). Acceptance: `art maps` still assembles surface via
      `_surface_kind`; nothing else writes one.
- [ ] Add the AO GEOMETRY guard to `_surface_kind` ([F3](forks.md#f3)): warn when G's opaque
      bbox differs from B's past `max(4 px, 6%)` per axis. Acceptance: fires on today's stale
      conifer AO, silent on the wolf's.
- [ ] Regenerate AO + surface for conifer, flora and wolf. Acceptance: the
      [I2](issues.md#i2) bbox sweep puts every AO inside the F3 tolerance and the guard stays
      silent across the run.
- [ ] LOOK at it in the client, lit (`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`).
      Acceptance: cavity darkening sits inside the silhouette and tracks the art, not a ghost
      oversized copy; before/after in `completed.md`.
- [ ] Record the tree-wide AO recommendation ([F7](forks.md#f7)): measured per-leaf cost ×
      216, and whether flat tiles want AO at all. Acceptance: a successor can act without
      re-deriving it.

## P2 — free the R lane

- [ ] `_surface_kind`: stop writing the emissive mask into R; R is 0 until P3
      ([F1](forks.md#f1)). Acceptance: every regenerated map has `R min == R max == 0` and no
      leaf reads `emissive.png`.
- [ ] Verify nothing reads `zdepth.R` or the composite presence bit BEFORE deleting either
      ([I4](issues.md#i4)). Acceptance: a recorded grep over `client/webgl/src` showing only
      the writes being removed.
- [ ] `mrtBakeShader`: delete `uEmissiveOn`, the `oDepth.r` relay and the `t.width > 1`
      gate; attachment 1's R RELAYS the leaf's R. Acceptance: shader compiles, world renders
      unchanged.

## P3 — extract the OUTER outline into R

- [ ] `split_layers.py`: split `_outline_mask()` into OUTER (components adjacent to a
      non-covered pixel) and INNER ([F4](forks.md#f4)). Acceptance: on conifer/1 the outer
      mask is the rim only — tier lines and trunk seams excluded.
- [ ] Emit the outer mask co-located as `outer_outline.<dir>.<part>.png`, like `occlusion`.
      Acceptance: one file per leaf that has a layer split; a leaf without one falls through
      to no file.
- [ ] Guard full-bleed masters ([I8](issues.md#i8)): coverage at (near-)the whole frame gives
      an EMPTY outer mask. Acceptance: every `biome-tile/**` leaf yields an all-zero mask and
      an untouched albedo.
- [ ] `_surface_kind`: write the outer mask into R, keeping the map RGB with no alpha.
      Acceptance: conifer/1's R is the rim, `biome-tile` R is black, G and B byte-identical
      to the P1 output.
- [ ] Measure and record the band: distance transform inside the outer mask → median
      thickness → the leaf's `meta.json` ([F5](forks.md#f5)). Acceptance: conifer, flora,
      wolf and human each carry a per-leaf width, not a constant.

## P4 — strip the rim from albedo, shrink presence under half of it

- [ ] `split_layers.py`: remove the OUTER outline from the residual/layers; INNER lines
      untouched. Acceptance: conifer/1 renders rim-free with its tier lines intact, compared
      against the P3 capture.
- [ ] `_surface_kind`: erode B by `max(1, round(median_band / 2))` px under the outer mask
      only ([F5](forks.md#f5)). Acceptance: the eroded edge follows the rim, coverage drops
      by ~half the band's area, no interior holes.
- [ ] `outline.py`: re-source the shadow silhouette from `surface.B` instead of the diffuse
      alpha ([I6](issues.md#i6)). Acceptance: a regenerated `meta.json` polygon set matches
      the eroded B, not the diffuse.
- [ ] Regenerate every `[thing.part.subframe]` rect with `subframe.py` and republish —
      regenerated, never hand-edited. Acceptance: `content/things.toml` diffs show only rect
      changes; the client loads without a subframe warning.
- [ ] Re-verify placement after the re-measure. Acceptance: captures of a two-part human and
      a facing-set wolf, each seated on its tile with the head in place.

## P5 — the client draws the line back (lands with P4 — [F6](forks.md#f6))

- [ ] `albedoBlitShader`: draw the outline from the MIXED `surf.r`, after the mix and depth
      resolve ([I10](issues.md#i10)), at the recorded width, behind a debug toggle.
      Acceptance: a hand-made R gives one leaf a rim that tracks it under pan/zoom.
- [ ] Prove the shadow cheat: shrunk presence + drawn rim leaves the silhouette visually
      where it was. Acceptance: a lit before/after at one camera with no rim gap and no
      findable shadow shrinkage.
- [ ] Flip the toggle on by default once the tree is rim-free. Acceptance: a full-scene
      capture with no sprite missing its outline.

## P6 — close the loop

- [ ] Regenerate the whole texture tree through the fixed pipeline and upload. Acceptance:
      216 surface maps with the F3 guard silent, R populated where art has a rim, B eroded,
      the client rendering the lot.
- [ ] `dev/textures` gets a `current/` snapshot naming what each map holds and which lanes
      are read, with a freshness stamp. Acceptance: `rd docs-check` green, stamp present.
