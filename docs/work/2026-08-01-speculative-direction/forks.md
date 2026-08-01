# Forks — speculative direction

## F1 — what "at rest" adopts, and when {#f1}

Options considered for the resting pawn:

1. **Snap to server facing the moment the spec lands** — reintroduces the pop the
   stream exists to remove (the landing row may still carry the pre-trip facing).
2. **Keep the last motion facing forever** — a pawn turned server-side while standing
   (an emote, a future interaction) never turns.
3. **Adopt server facing at rest, but only from rows that ARRIVE while resting**
   (chosen): the landing row itself never turns the pawn (its facing is stale by
   construction); any later row that changes facing while the pawn stands is a real
   turn and applies. Motion facing survives the landing; genuine turns still land.

## F2 — rendered delta over spec-space facing {#f2}

`walkGreedy` already reports a facing, but it is spec-space: reseeds snap its `from`
under the walk, and the chase moves the sprite along its own per-axis path. The eye
follows the RENDER, so the render's delta is the only source that cannot disagree with
what is seen. The greedy facing survives only as the first-frame aim (no delta exists
yet).
