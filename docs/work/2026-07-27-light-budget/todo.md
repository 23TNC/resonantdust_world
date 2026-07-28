# Plan — a prioritised light-update budget

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.** Every phase must hold these:

- **No single gather draw exceeds the budget in tile-light pairs.** This is the stability property — an
  unbounded draw is what trips the GPU watchdog.
- **Cold lights are serviced within a bounded number of frames**, and that bound is measured, not
  assumed. "Hot usually wins" is not starvation-free.
- **corridor↔brute identity = 0 differing texels** once the queue has drained. Budgeting changes WHEN a
  tile is computed, never WHAT it computes.

**Fixture:** the calibrated orbit harness from
[plane-intersection](../2026-07-27-plane-intersection/completed.md) — one moving light, 6-tile orbit,
zoom 1, 240 frames, cold gather draw discriminated by RENDER TARGET, fixed frame count, phase from the
frame index. Do not use a frozen light; it looked stable to ±0.001 while measuring nothing.

## P0 — Measure the dirty set before rationing it

- [ ] Instrument pairs-per-frame (Σ over dirty tiles of that tile's light count) and log its distribution over an orbit, so the budget is set from data rather than from the 512 guess.
- [ ] Record the worst case: 3 content torches at reach 12, plus a cold start, in pairs and in ms. The cold start is the case that currently loses the context.
- [ ] Confirm the flat ~0.0015 ms per pair holds with SEVERAL lights covering one tile, not just one. The budget's whole premise is that a pair is a constant unit.

## P1 — Priority plumbing (no budget yet, no behaviour change)

- [ ] Give each queued rect in `pendingRects` a priority field alongside its existing class field, and have `markLightDirty` compute it from the light.
- [ ] Score: hot outranks cold; within a class, nearer the camera anchor outranks further. Keep it a single comparable number so admission is a sort, not a rule cascade.
- [ ] Where rects overlap, a tile takes the MAX priority of the rects covering it — a tile dirtied by both a hot and a cold light is hot work.
- [ ] Verify no behaviour change: with the budget disabled, corridor↔brute identity and the frame cost must match the pre-change build.

## P2 — The budget

- [ ] Admit dirty tiles in descending priority until the pairs allowance is spent; leave the rest queued rather than dropping them. Deferred ≠ discarded — a dropped tile keeps stale light forever.
- [ ] Track the deferred set across frames and drain it, so a tile that misses one frame is still guaranteed to be computed.
- [ ] Set the initial allowance to 512 pairs (~0.9 ms, ~5 % of a 60 fps frame) and expose it as a single named constant next to `BAKE_BUDGET`, so the two rations are legible together.
- [ ] Assert the bound: no frame's gather may exceed the allowance. Log a counter when the queue is non-empty so backlog is visible rather than silent.

## P3 — Starvation guarantee

- [ ] Age deferred tiles: a tile's effective priority rises with each frame it waits, so cold work is eventually admitted no matter how much hot work arrives.
- [ ] Choose the ageing rate so a cold tile's worst-case wait is a stated number of frames, and write that number down — the guarantee is the point, not the mechanism.
- [ ] Measure the worst-case wait under adversarial load (a hot light orbiting continuously while a cold light also moves) and confirm it matches the stated bound.

## P4 — The prim-facing lever

- [ ] Add an explicit priority boost to a light that a prim can set, defaulting to zero so nothing changes until something uses it.
- [ ] Fold the boost into P1's score rather than special-casing it, so a boosted cold light can outrank an unboosted hot one — that is the point of having the lever.
- [ ] Leave it unused by content, and say so in the code: this is a tool being built ahead of the gameplay that needs it (user), not a feature with a current caller.

## P5 — Verify

- [ ] Load the pre-plane-intersection build with the budget in place and confirm it now survives its cold start. That build currently loses the context before presenting a frame, and it is the sharpest test that the bound works.
- [ ] Re-run corridor↔brute identity after the queue drains and require 0 differing texels.
- [ ] Measure the visible cost of deferral: how many frames a tile can hold stale lighting under normal motion, and whether that reads as lag or as a glitch.
- [ ] Profile the 3-torch scene against the unbudgeted build and record both the frame cost and the worst single draw.
