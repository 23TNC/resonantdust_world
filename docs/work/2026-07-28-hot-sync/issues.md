# Issues — hot-sync

## I1 · The remaining misalignment is NOT sync — it's caster-silhouette GEOMETRY (n/s frames)

Found during the user's live check after delivery. Evidence chain: (a) the user observed the
wolf misaligned with its shadow in motion (close-up: a dark wolf shape offset from the
sprite + horizontal streaks); (b) at REST the streaks persist (so not the motion quantum);
(c) **`__gather.rebakeAll()` reproduces them** — a full forced re-render draws the same thin
horizontal bars, so they are LIVE GEOMETRY, not stale bake. The e/w resting wolf casts a
correct attached silhouette; the n-facing wolf emits 1-tile horizontal slivers displaced
from its body. Prime suspect: the n/s wolf DEFS' geometry (W/H, tight bbox, frame offsets)
disagreeing with the actual atlas frame content — the concurrent art-128 session resized
wolf masters today (the "giant beige wolf" symptom), and `casterCover`/`receiverCover`
sample the silhouette through the def's packed geometry; a mismatch slices garbage rows.
NEXT: audit `definitionFor`'s emitted W/H/span/offsets for the wolf's s/n stems against the
co-packed frames (post-normalization), and re-verify the e/w frames too (the user's original
close-up was an e/w wolf with a displaced dark shape — possibly the same defect at smaller
magnitude). Hot-sync's OWN deliverable (one dirty, lockstep) is measured-good and unaffected.

_Defects found during execution land here. Known input: the desync itself is DIAGNOSED in the
README (three independent hot dirty gates + a frame-order skew) — P0 turns the diagnosis into
before-numbers._
