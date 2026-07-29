# Issues — hot-sync

## I1 · The trailing dark ghost — TWO POSITION AUTHORITIES (fixed by P3)

Found during the user's live check after delivery; my first diagnosis was WRONG (I blamed
n/s caster geometry off the streaks — the user corrected: **the streaks/shadows are fine**;
the bug was a dark wolf-shaped ghost trailing the MOVING wolf ~0.5 tiles). Root cause: the
sprite baked from the warm prim's CPU **float** position while every lighting consumer read
the billboard record's **unit-quantised** position — two authorities for one datum, so the
drawn wolf and its lit/shadowed footprint could disagree by however much the paths skewed
("draw, shift position, draw" — the user's read). The def geometry itself checked out clean
(wolf tight box 48×112 in a 128 frame). FIX = P3: the record is the position authority —
sub-unit anchor lanes + the CPU snapping the hot prim to the record-decoded anchor at record
write. Verified live: no ghost across a multi-trip tracked soak (n and e/w facings, lit
ground), counters 100/100 dirty-calls→record-changes, backstop 0, 120 fps.

_Defects found during execution land here. Known input: the desync itself is DIAGNOSED in the
README (three independent hot dirty gates + a frame-order skew) — P0 turns the diagnosis into
before-numbers._
