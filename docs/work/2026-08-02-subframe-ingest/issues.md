# Issues — subframe at ingest

_What is broken, suspect, or unverified. Numbered so decisions and commits can cite them._

## I1 — How much of an atlas frame is actually art? MEASURED — 0.461 {#i1}

The crop's second payoff is resolution: a letterboxed master spends texels on transparent margin at
**every** lod, and the atlas is the biggest texture cost in the client (`SQUARE 128` art maps measured
~321 MiB, `square-128` P0).

**Answered in P0: the corpus average is `0.461`** — more than half of every atlas frame is
transparent margin, so the crop is a real resolution and memory win rather than a correctness change
only. The per-stem table is in [`completed.md`](completed.md); the worst stem
(`pawn/human/female/11/e.1`) is `0.190` opaque and the best (`biome-thing/default/flora/e`) `0.742`.

P4 re-measures after the crop. The `0.461` here is the number that claim is made against — and it
came in far enough from 1.0 that the plan does **not** have to retreat to "correctness only".

## I2 — Linked/grid stems already crop — DECIDED, `internal_padding` retires ([F6](forks.md#f6)) {#i2}

`<stem>/l` stems carry a DSL `internal_padding` and the resolver *already* trims each linked cell by
it (`setLinkedPad`, `texture-generalization`). A grid stem is also explicitly excluded from
`sprite_scale` today — `packCoPack` warns and packs unscaled when `entry.grid` is set. So a grid stem
had a per-cell crop that was not the per-stem subframe, and applying both would shift every autotile
cell by the pad.

**Resolved by deleting the older of the two** (user, 2026-08-02: *"we can likely drop
internal_padding and leverage this subframe method to accomplish the same thing unifying our code and
variables"*). A uniform inset **is** a uniform subframe, so the pad is a special case of the thing
that replaces it. See [F6](forks.md#f6).

## I3 — `ppu` is derived from the frame, and the frame's meaning changes {#i3}

The def's SEED lane carries `pxPerUnit = frame.w / (span · 16)`, and `silhouetteHit` addresses the
surface quadrant as `(fy + frameUnits) · ppu + oy`. Both assume the frame's px map linearly onto the
span's units.

After the crop that is still true — the quadrant is still `quadN` square and still spans `span` tiles
— **but only because [F2](forks.md#f2) keeps the quadrant square**. Any drift toward a non-square
quadrant breaks `ppu` silently, and the symptom would be a sampling offset that looks exactly like
the bug this stream is fixing. Recorded so it is not re-diagnosed from scratch.

## I4 — The crop reverses a normalisation the art pipeline does deliberately {#i4}

`texture-pow2-normalization`: masters are **square pow2 with the subject letterboxed**, and the
aspect plumbing was removed on purpose. This stream does not undo that — the masters on disk stay
square pow2, and the lod ladder still needs them to be.

The crop is a **GPU-side ingest step only**. Stated here because "we crop the art" reads like a
pipeline change and is not one; `bin/art` keeps emitting exactly what it emits today
([F1](forks.md#f1) authors the subframe *against* those masters).

## I5 — The bbox does not disappear; it changes job {#i5}

`computeSpriteBBox` (from the SURFACE map's B coverage) currently derives at runtime what the DSL is
about to author. After this stream the runtime must **not** consult it for geometry — that is the
duplication returning.

But it is still the right way to *propose* a subframe to an author. The useful end state is a
dev-time tool (`bin/art`, or a `__bbox` probe) that prints the measured bbox for pasting into the
`.rd`, with the runtime path deleted. Recorded so the deletion does not take the measurement with it.

## I6 — Nothing verifies that an authored subframe matches its art {#i6}

Once the number is authored, a regenerated master can move the art inside its frame and the `.rd`
will not know. The failure is silent and looks like a lighting bug — the same class as
[normal-frames I5](../2026-08-02-normal-frames/issues.md#i5).

A cheap guard: at ingest, compare the authored subframe against the measured bbox and `console.warn`
past a tolerance. Cheap because the bbox is already computed at decode. **Not yet decided** whether
that lands in this stream or is left as a follow-up.

## I7 — The derived bbox is not LOD-STABLE {#i7}

Found while measuring P0. `biome-thing/default/flora/e` reported `bb.fy = 0.125` in one session and
`0.109` in the next, on unchanged art.

`computeSpriteBBox` runs on **whichever lod decoded first** (`ensureCoPack` guards with
`if (surf && !this.spriteBBox.has(stem))`), and a 16-px master resolves the silhouette's edge to
1/16 of the frame where a 128-px one resolves it to 1/128. So the "same" bbox is a different number
depending on how the session happened to stream.

This is independent evidence for [F1](forks.md#f1) and the stream as a whole: a derived geometry
number that changes with streaming order cannot be the thing placement is registered against. An
authored fraction is identical at every lod by construction.

It also sharpens [I6](#i6): the ingest-time warning must compare the authored subframe against the
bbox **at a stated lod**, or it will fire spuriously on the low tiers.
