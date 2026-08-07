# Issues — human-pawns-redux (anticipated inventory)

## I1 — the subframe registration touches the shared crop pipeline {#i1}

The I11 fix registers mover rects into the same `setSubframe` store cold things use.
Hazards: the store keys by STEM (the per-part stems `…/<variant>/e.1` must match what
`moverSlotTexture` resolves, including the variant-folder fallback), the west facing is a
MIRRORED east (`r3` — never a stored stem), and the wolf's existing render must not shift
except where its authored rects were silently ignored (its crops may legitimately CHANGE —
compare before/after captures and judge, don't assume identity).

## I2 — offered-but-refused is a CLASS, not just drink-on-humans {#i2}

F4 closes the instance, but the general hazard stands: the menu filter knows predicates and
location, not effect-target presence (a satisfy against a need the pawn doesn't carry). If a
future carrier hits this again, the conversation is corpus validation (a kind whose
interactions' satisfy needs aren't in its `needs` list could refuse at LOAD), not menu
special-cases. Recorded for that day.

## I3 — head placement numbers are TOML now; judge by LOOKING {#i3}

The old tuning (`pawns.rd` `0.625`/`−1.15`) is gone; the TOML says head `scale 0.5`,
`offset.z 0.87`, `depth 1.0`, default anchors `(0.5, 0.5)`. The carried-piece path DERIVES
the head's elevation from the gap to its carrier (`recordSync` — not the authored value
directly), so placement is judged in the browser per [[albedo-outlines-by-design]]'s rule:
look at the client, tune the TOML, re-bless golden if sim-visible fields move (they
shouldn't — parts are art).

## I4 — the outline set covers the BODY only {#i4}

`primIdOf` returns `parts[0].id`, so a selected human outlines its body and the head floats
unmarked. The fix belongs in `syncOutlines` (outline every part of a selected pawn), not in
`primIdOf` (whose single-carrier answer other callers rely on).

## I5 — the chat spawn is a DEV door: spam mints many {#i5}

CREATE's replay ledger keys `(event_reference, index)` — idempotent per event, but every
command invocation is a NEW event, so ten `spawn`s mint ten humans. Accepted (dev posture,
same as the wolf adopt-race dirt); the command echoes the minted program so the operator
sees what they did. Ownership/cleanup is the standing recorded successor.

## I6 — golden + registry churn from `needs = ["thirst"]` {#i6}

The humans' sim fingerprint changes (needs join it), the golden thing-tables row changes,
and fresh mints get a thirst row + the derived conditions. Same re-bless discipline;
dev worlds re-mint.

## I7 — accumulated dev dirt: three wolves and maybe a ghost fixture {#i7}

The live world carries three adopt-race wolves; whether the original P4 fixture
(`0x30800005`) survived the wipes is a runtime question. The cold-boot drill should state
what actually exists rather than assume; a full-wipe redeploy remains the clean-slate lever
if the pond gets crowded.

## I8 — the retired `layer` lane is still WRITTEN {#i8}

VARIABLES retired the prim `layer` lane ("carried the pawn part slot and nothing ever read
it back") but `MoverLayer` still passes `layer: i` and `SquareCache.addPrim` still copies
it. Delete the write with the copy-site comment (the addPrim explicit-copy gotcha —
[[pawn-render-delivered]]), and sweep `SquareCache`'s stale "human-pawns P5" doc note.

## I9 — carried successors from pawn-part-placement (recorded, not built) {#i9}

What F5's closure carries forward without building here: per-part LIGHTING attachment (the
head as a caster/receiver piece — `claimPieceSlot` died in the lighting strip; the current
carried-piece record covers geometry only), footprint/occupancy for multi-tile pawns, and
the capture-comparison harness its plan wanted. Each is a later stream's item; this stream
only keeps their names alive.
