# Forks — def frame/anchor rework

_Decision points + options + which we chose + why. Chronological._

---

## F1 · ppu needs the frame's world span — `frame_span` u3 added {#f1}

**2026-07-23 — AMENDMENT to the user's layout, pending P0 ratification.**

`frame_lod` alone cannot yield px-per-unit: a 32px frame is a 1-tile sprite at 2 px/unit **or** a
2-tile sprite (the tree) at 1 px/unit. `ppu = 2^frame_lod / span_units` requires the span.

- **Chosen:** store **u3 `frame_span`** (log2 tiles, 2⁰..2⁴ = the 16-tile zone ceiling) in RED's
  tail. Self-contained def; integer shifts in the shader.
- **Alternatives:** (a) derive span from the prim's DSL footprint at gather time — rejected: the
  def stops being self-contained and P5 wants size OFF the DSL anyway; (b) restrict all frames to
  1-tile span — rejected: trees are 2-tile.
- Also corrected: ppu **doubles** per lod (`2^(lod−4)` for 1-tile), not `lod − 3`; GREEN reserve is
  u12 (10+10+12 = 32 — the proposal's u14 summed to 34).

## F2 · Nudge stored at the current lod's px scale {#f2}

**2026-07-23 — chosen.**

`nudge_x/y` are px at the **resolved lod** (bound: 2 units × max ppu 1024 = 2048 → u11). On a LOD
swap the def compare-write recomputes nudge with the new frame — same cadence that already rewrites
frame_x/y/lod. Alternative (nudge in fixed sub-unit fractions, lod-independent) rejected: costs
precision at high lod for no write savings.

## F3 · prim_data position semantics under anchors {#f3}

**2026-07-23 — OPEN (P3).**

Today `primDataFor` precomputes the base-centre (x + w/2, y + h) into `position_anchor_reference`.
With `frame_anchor` in the def, the natural model is: `prim_data` holds the prim's **reported x/y**
verbatim and the def's anchor places the bbox. Decide at P3 whether to (a) switch prim_data to
reported-x/y (cleaner, one meaning), or (b) keep base-centre and treat anchor as display-only.
Leaning (a); touches `primDataFor` + the gather's `A` usage + bucketing.
