# Deviations — pawn part placement

_Log a deviation from the plan AT THE MOMENT of deviating, with the reason. "Less churn" is never
a reason._

## D1 — P1's first item was rewritten mid-phase; its acceptance criterion was wrong
_2026-07-30_

The item read *"Apply slot 0's own `scale` to the carrier box in `MoverLayer`, per the re-resolved
[F3](forks.md#f3). Acceptance: authoring `0.8 &body.scale set` makes the body's read-back `w`
102.4, not 128."*

Doing that would have given the BODY the defect the head already had. [I6](issues.md#i6) proved a
drawn box that disagrees with the def's pow2 frame span casts a silhouette scaled by the ratio —
102.4 px of body inside a 128 px card is 1.25×. The criterion demanded the bug.

Rewritten to the [F4](forks.md#f4) shape: the scale goes pre-atlas and the box stays at
`span × SQUARE`, so the acceptance flips to `w` staying **128** while the ART measures 0.8 tiles.
Recorded here rather than silently re-worded because the number in the old criterion is the exact
thing that changed.

I also **removed `body.size` from `pawns.rd`** (both sexes). It was inert for the drawn box
([I4](issues.md#i4)) and `size` is documented as superseded by `span` + `sprite_scale`; leaving a
`1.5` there would keep implying the body draws 1.5 tiles tall when it draws 1. Its one live reader
is `maxCardTiles` (see [I8](issues.md#i8)), where the human's contribution correctly becomes 1.
