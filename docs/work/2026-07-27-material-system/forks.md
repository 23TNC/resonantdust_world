# Forks — material system

_Decision points, options, which we chose and why._

## F1 — where does the colour variation go? {#f1}

The user's explicit open question ("the color variations could flow along the normal maybe? I'm
not sure how we might properly place the color"). This fork stays OPEN until the by-eye A/B in P4;
all candidates are cheap enough to build behind one switch:

- **(a) UV-space noise** (exists — `sampleSpace 0`): variation rides the sprite. Stable, but
  placement is arbitrary relative to the drawn structure.
- **(b) World-space noise** (exists — `sampleSpace 1`): pinned to the ground; good for terrain,
  wrong for a standing sprite (variation shears across the card).
- **(c) Detail-field-keyed**: the SAME tiling field that perturbs the normal also drives the hue
  sample — colour and relief share one cause, so a needle clump is darker/lighter AS a clump.
- **(d) Normal-keyed**: hue varies with the (detailed) normal's facet direction — reads as
  orientation-dependent sheen/pigment.

**Lean: (c)**, because coherence between relief and colour is precisely what makes structure read
at sprite scale; (d) risks double-counting with the real per-light N·L. The user picks at P4 with
screenshots of each; the record of the pick lands here.

## F2 — how detail blends onto the base normal {#f2}

- (a) Linear add + renormalise: cheapest, flattens at grazing angles.
- (b) UDN (add xy, keep base z): cheap, decent for mild detail.
- (c) RNM (reoriented normal mapping): correct rotation of detail into the base's frame; a few
  more ALU, category-standard.

**Chosen: (c) RNM.** The whole point is fixing plastic normals — the correct-looking blend is the
feature; the cost sits in the budgeted bake, not per frame. (b) is the fallback if RNM shows
artefacts with Laigter's opaque flat-up backgrounds.

## F3 — the seed lane in `billboard_data` {#f3}

Record has `u14 reserved` in B (bits 0–13) and `u32 reserved` A. Options: (a) `u8 seed` in B's
low 8 (keeps A whole for a future big consumer); (b) a lane in A.

**Chosen: (a)** — 256 variation steps is ample (the bake hashes it onward), and A stays intact as
the record's one large reserve. VARIABLES.md gets the layout FIRST; code conforms after (the
authority rule). The stamp source is `cellSeed(tx, ty)` quantised to u8 — deterministic per cell.

## F4 — the needle field {#f4}

- (a) Reuse `strand` (exists): directional streaks — close, but tuned for fur/grain.
- (b) Append a dedicated `needle` field to `bin/lib/noise_fields.py` + the atlas: short
  high-frequency directional dashes with orientation jitter.

**Chosen: (b), appended** (atlas rows are positional — append-only is the catalogue's own rule).
`strand` stays untouched for its existing users; the needle look gets its own dial. If (b)'s
first cut reads poorly, (a) is the in-place fallback during tuning.
