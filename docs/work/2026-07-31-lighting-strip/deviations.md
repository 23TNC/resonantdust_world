# Deviations — strip the lighting + shadow system

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

## D1 — P0's four measurement items struck, on the user's instruction (2026-07-31)

**What the plan said.** P0 recorded the per-pass ms at N=1 and N=16, resident RT bytes, a capability
inventory and the structural findings — all before deleting anything, on the principle that a
measurement not taken before the code goes is gone for good.

**What happened.** The user, after watching the timing harness fight the browser: _"Alright lets skip
P0."_ The four items are **struck from the plan**, not parked.

**Why this is the right call and not a shortcut.** The principle was sound; the *instance* was
over-invested. Reading it back honestly:

- **The images were the irrecoverable part**, and they are captured and committed. Everything else was
  documentation of a system that git still holds in full.
- **P1 produces the number that matters anyway.** "The frame with the lighting passes dark" is one
  frame-level measurement and it prices the whole system — which is what the replacement's budget
  needs. The per-pass breakdown was finer detail than the decision requires.
- **The harness cost was real and rising.** Five page reloads chasing WebGL timer-query retirement in
  a backgrounded tab (see [I7](issues.md#i7)); the last attempt lost 740 of 810 queries. That is a
  measurement problem, not a lighting problem, and it was consuming the stream.

**What was salvaged rather than lost** — the one clean measurement, recorded in `completed.md`: at
N=1 reach 16, gather **0.603 ms**, lighting 0.119, blit 0.378, total **1.111 ms**. It cross-checks
against the prior stream's independently-measured 0.610 ms gather, so the rig was correct.

## D2 — F5 was wrong: the primitive graph is NOT load-bearing for the unlit renderer (2026-07-31)

**What the plan said.** [F5](forks.md#f5) chose to *split* `coldShadowData.ts` — "keep the
primitive-graph writer, delete the lighting-only paths" — on the stated grounds that _"sprites, z-depth
and selection all read it"_.

**What the code says.** Verified at P2, before touching anything:

- **Sprites and z-depth do not read it.** `mrtBakeShader.ts` and `SquareCache.ts` contain no `uData`,
  no `usampler` — the G-buffer bake never samples the data texture. The unlit render is completely
  independent of the record layer.
- **Exactly one non-lighting consumer exists**: `Viewport.thingAt()` (click-to-select) calls
  `shadows.tightBoxFor()`, a six-line wrapper over `coldData.definitionFor` + `tightBoxOf`.
- **And that wrapper's data has another source.** `TextureResolver.opaqueBBox(stem)` already returns
  the silhouette bbox as frame fractions — which is what the tight box was derived from in the first
  place, before being quantised to even units for the shadow card.

**What I did instead.** Reimplemented `tightBoxFor` in `Viewport` directly against the resolver, and
deleted **both** `shadowGather.ts` and `coldShadowData.ts`. Selection keeps working with **no
capability loss** — the unquantised fraction is strictly *more* precise for hit-testing than the
even-unit tight box it replaces.

**Why this is the right call and not scope creep.** F5's split existed to keep a record layer alive
that nothing in the unlit renderer reads, and that
[`2026-07-31-lighting-rework`](../2026-07-31-lighting-rework/README.md) P1 replaces wholesale with
`prim_data` / `definition_data`. Carrying 1 348 lines through two phases to delete them in a third is
motion, not progress — and it would have left a half-retired band layout in `VARIABLES.md` for the
rework to trip over.

**Consequence for P3.** "Retire the data-texture bands" becomes a documentation-only phase: the
writers are already gone, so P3 removes the band specs from `VARIABLES.md` rather than un-wiring code.
