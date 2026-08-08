# Completed — surface channels

_The verification log: what landed and how it was checked. The investigation that produced the
plan is in [`issues.md`](issues.md) I1–I4._

## 2026-08-08 · P1 (part) — the pipeline stops producing wrong-sized and split maps

Driven by a user bug report against `art remaster`: numbered `surface.e.0-1.png` /
`surface.e.0-2.png` files, a surface map that stayed wrong, and albedos coming back 128 instead
of the proper 256. All three were one causal chain, and both new links are now closed.

**The span reader was stale ([I11](issues.md#i11)).** `def_span.py` keyed on a part's explicit
`texture = "<stem>"`; the corpus stopped authoring it when defs went registry-numbered with a
derived taxonomy, so `scan()` returned empty and exited 2. `_span_side` then fell back and
`cmd_split` sized every leaf to one tile.

- Reproduced before the fix: `def_span biome-thing/default/conifer` → exit 2, *"content/things.toml
  yielded no texture-bearing defs — the corpus moved or this reader is stale"*.
- Fixed by deriving `<type>/<subType[0]>/<kind>` in `_things`, with an explicit `texture` still
  winning where authored.
- Verified: `def_span --all` now resolves exactly the five art-bearing stems (conifer 2/256, flora,
  wolf, human male + female) and skips every `texture = "white"` placeholder; `_span_side
  biome-thing/default/conifer` → `256`, `pawn/animal/wolf` → empty (no span authored, falls back),
  `biome-thing/default/shrub` → empty (placeholder).
- Note recorded in the issue: `meta.json` still read `span 2 / square 256` throughout, because
  `meta.py` merges keys forward — July's sidecar values rode through today's rewrite while the
  pixels halved. The sidecar and the art disagreed and only the art was wrong.

**A mismatched lane split the surface map ([I12](issues.md#i12)).** With the diffuse at 128 and the
stale `occlusion` at 256, `_surface_kind`'s `-combine` could not combine and wrote the three lanes
as `surface.e.0-0/-1/-2.png`, leaving the real `surface.e.0.png` at its July bytes — which is what
"still seeing incorrect surface" was.

- Confirmed on disk: 27 orphans across the nine conifer leaves, sizes 128 / 256 / 128 matching the
  R / G / B lanes exactly, beside a 256 `surface.e.0.png` dated 07-28.
- Fixed by resizing G to the diffuse's geometry (as R already was) plus a post-write check that
  names the split and clears the orphans.
- Verified: `art surface biome-thing/default/conifer` produces one 128×128 map per leaf, zero `-N`
  files; the 27 orphans deleted.

**AO generation works again ([I1](issues.md#i1)).** `depth_ao.py` discovered and wrote the pre-fold
flat `diffuse.png` / `occlusion.png` names, so `art ao` matched nothing under the folded layout and
no AO had been regenerated since that migration.

- Discovery and all four write paths now go through `texpath` (`find_maps` / `sibling` /
  `leaf_file`), the same shape source of truth `bin/art` uses.
- Verified end to end: `art ao biome-thing/default/conifer/1 --no-pack-normal` wrote
  `occlusion.e.0.png`; the regenerated map is 128×128 (matching its diffuse) with a **white**
  background — the `height_to_ao` contract of 1.0 outside the mask, where the stale file had
  `gray(16)` and was demonstrably not produced by this code.
- Geometry now agrees: AO bbox `57x95+36+18` against coverage `51x90+39+20` — a ~3 px halo, the
  same margin the wolf always had. Before the fix the same leaf measured 1.46× × 1.40×.

**The second surface writer is gone ([F2](forks.md#f2)).** `depth_ao.py --surface` wrote
`R=height, G=AO, B=zeros, A=silhouette` — a layout contradicting `_surface_kind`'s, whose `B=0`
renders a sprite invisible. Flag, branch and its `not args.surface` guard deleted;
`_surface_kind` is the only assembler.

**Still owed in P1:** the [F3](forks.md#f3) bbox guard (the resize fixes the split, not the scale —
a 256 AO squeezed into 128 keeps its wrong subject extent), the regeneration across flora and wolf,
the look in the client, and the tree-wide AO recommendation. The masters remain at 128 until the
user re-runs `remaster` under the fixed reader; that rewrites masters from the source sheets, so it
is theirs to run.
