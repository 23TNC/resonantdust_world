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

## I8 — A DSL store node cannot be both a Map and an Array {#i8}

Found implementing [F9](forks.md#f9). Authoring `&thing.subframe.x` *and* `&thing.subframe.9.x`
looks natural and **silently drops the second write**.

`seg_of` turns an all-digit path segment into `Seg::Idx(i)` and anything else into `Seg::Lit`. The
first write makes `subframe` a `Cell::Map` (key `"x"`); the second then walks `Seg::Idx(9)` into that
Map, hits `Cell::Map(m) if *i < m.len()` — `9 >= 1` — and falls through with **no write and no
error**. The value is simply gone, and the field reads as its default.

**Resolved by never mixing the two under one node**: rotation keys are `r0`..`r15`, plus the
`s`/`e`/`n`/`w` aliases for `r0`..`r3`. All `Seg::Lit`, so `subframe` stays a Map throughout and the
non-indexed `subframe.x` default coexists with `subframe.r9.x`.

**Worth knowing beyond this stream.** Any DSL variable that wants both a scalar default and indexed
overrides under one name has this trap, and it fails silently — which is the worst way for a content
authoring error to fail. If bare-numeric indices are ever wanted, they need their own sub-node
(`subframe.rot.9.x`), not a sibling of the scalar keys.

## I9 — The crop magnifies: the DRAWN BOX must derive from the subframe too {#i9}

Found the moment the crop went live (P2). The subframe changes what the frame *contains* — the art
now fills it instead of sitting letterboxed inside it — but the drawn box is still sized from `size`
/ `span` / `sprite_scale` as though the frame still held the margin. Net effect: every cropped sprite
draws at roughly `1 / subframe.h` its correct size. The conifers came back about 2× too tall.

**This is not a bug in the crop; it is the other half of it.** The old model had one number
(`size`) meaning "how much world does this frame cover", and the frame was mostly margin, so `size`
was implicitly absorbing the letterbox. Removing the margin without telling the draw path leaves
`size` over-stating the art by exactly the margin it used to include.

Two candidate fixes, to settle before writing either:

1. **Scale the drawn box by the subframe** — `drawn.h = size · sub.h`, `drawn.w = size · sub.w`.
   Keeps the corpus's `size` meaning "the frame's world span" and derives the art's span from it.
   Nothing in the corpus changes.
2. **Re-author `size` per kind** to mean the ART's span directly, now that the frame is the art.
   Cleaner conceptually, but it re-authors every kind and silently breaks any kind not re-authored.

(1) is almost certainly right — it is the same "derive, do not re-author" reasoning that made the
subframe a crop rather than a master change ([I4](#i4)), and it keeps unauthored kinds identical
(their subframe is the whole frame, so the factor is 1).

**Also unresolved by (1):** the ANCHOR. Once the box is right, the art's bottom sits at the box's
bottom only if `sprite_anchor.y = 1` is honoured in the *placement*, which is [F3](forks.md#f3)/P3's
question — and the shadow-gap symptom the user reported is that anchor, not this scale.
