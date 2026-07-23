# Forks — def frame/anchor rework

_Decision points + options + which we chose + why. Chronological._

---

## F1 · ppu needs the frame's world span — `frame_span` u4 added {#f1}

**2026-07-23 — RATIFIED (user).**

`frame_lod` alone cannot yield px-per-unit: a 32px frame is a 1-tile sprite at 2 px/unit **or** a
2-tile sprite (the tree) at 1 px/unit. `ppu = 2^frame_lod / span_units` requires the span.

- **Ratified:** **u4 `frame_span` = span tiles − 1**, capped at one zone. User widened my u3-log2
  proposal to u4 so the field's width is **`log2(ZONE_DIM)`** — no magic number: bumping the zone
  size drags the field forward with it. Valid values are pow2 tiles (integer ppu); the writer
  rounds up + warns.
- **Alternatives rejected:** (a) derive span from the DSL footprint at gather time — def stops
  being self-contained, and P5 wants size OFF the DSL; (b) restrict frames to 1-tile span — trees
  are 2-tile; (c) u3 log2 encoding — magic width unmoored from ZONE_DIM.
- Also corrected: ppu **doubles** per lod (`2^(lod−4)` for 1-tile), not `lod − 3`; GREEN reserve is
  u12 (10+10+12 = 32 — the proposal's u14 summed to 34).

## F2 · Nudge stored at the current lod's px scale {#f2}

**2026-07-23 — chosen.**

`nudge_x/y` are px at the **resolved lod** (bound: 2 units × max ppu 1024 = 2048 → u11). On a LOD
swap the def compare-write recomputes nudge with the new frame — same cadence that already rewrites
frame_x/y/lod. Alternative (nudge in fixed sub-unit fractions, lod-independent) rejected: costs
precision at high lod for no write savings.

## F4 · nudges signed — RATIFIED as u12 ±2048 + nudge anchors {#f4}

**2026-07-23 — RATIFIED (user, widened).**

The proposal assumed top-left grid bias ⟹ centering always nudges **right** (unsigned). False when
the opaque run sits deep in its first grid cell with little slack: e.g. ppu=4, in-window offset
r=3 px, slack=3 px → centered start needs a shift of `slack/2 − r = −1.5` px — **leftward**.

- **Ratified:** user widened both nudges to **u12 with +2048 bias** — full either-direction range
  (2 units × max ppu 1024 = 2048 in both signs), so ANY alignment (left/center/right, top/bottom)
  is expressible regardless of where the opaque run sits in its unit. Plus **u2 nudge_anchor_x/y**
  recording WHICH alignment the stored nudge encodes — defaults x = 1 (center), y = 2 (bottom, the
  shadow base) — future nudging operations write other anchors. ALPHA is now FULL (12+12+2+2+4).

## F3 · prim_data position semantics under anchors {#f3}

**2026-07-23 — OPEN (P3).**

Today `primDataFor` precomputes the base-centre (x + w/2, y + h) into `position_anchor_reference`.
With `frame_anchor` in the def, the natural model is: `prim_data` holds the prim's **reported x/y**
verbatim and the def's anchor places the bbox. Decide at P3 whether to (a) switch prim_data to
reported-x/y (cleaner, one meaning), or (b) keep base-centre and treat anchor as display-only.
Leaning (a); touches `primDataFor` + the gather's `A` usage + bucketing.
