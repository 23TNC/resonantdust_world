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

## I10 — F9 collapsed TWO axes into one: a facing is not an index {#i10}

> "Our wolf is being cut off, I didn't think we passed anything for wolf?" — user, 2026-08-02

We did, and that is the bug. **[F9](forks.md#f9) is wrong as built**, and this is the correction.

F9 claimed one `0..15` space serves sprite facings, linked autotile cells and variants alike. It does
not, because **a facing and a cell are different axes and a single kind needs both**. The wolf has 3
mastered facings *and* 15 east variants.

The evidence is in how a stem resolves. `thingTexture` builds `<base>/<facing>` and passes
`cell = variant`; `moverSlotTexture` builds `<base>/<variant>/<facing>` and passes no cell at all.
Either way **the facing is already in the STEM** — so an index alongside it can only mean the cell.

The wolf authored `subframe.e/s/n`, the loader collapsed those to indices 1/0/2, and the resolver
then applied index 0 — the **south** rect, a narrow tall box — to the wolf's **east** art. Hence a
wolf cut in half.

**The right model, unimplemented:**

| axis | keyed by | authored as |
|---|---|---|
| facing | the **stem** (`…/wolf/e`) | `&thing.subframe.<e\|s\|n>.*` → registered against `<base>/<facing>` |
| cell / variant / autotile | the **index** `0..15` | `&thing.subframe.r<n>.*` → registered at that cell |

So `s`/`e`/`n` must stop being *aliases* for `r0`/`r1`/`r2` in the loader and become their own axis.
They are not two spellings of one thing — that was the mistake, and it is the same
two-things-in-one-slot error the whole stream exists to remove, committed by me while removing it.

**Mitigation applied now:** the wolf's subframe authoring is removed from the corpus, so it falls
back to the whole frame and renders correctly again. Conifer and flora are **unaffected** — they
author `r<n>` VARIANT indices on a single facing, which is the axis the code actually implements, and
they are the case the user confirmed "mostly works".

**Do not restore the wolf's rects until the loader carries the two axes separately.**

## I11 — Mover stems are registered as if they were COLD stems — OPEN (the wolf symptom was STALE) {#i11}

> "The wolf is textured as a tree, and moving at light speed." — user, 2026-08-02

**Not a revert.** Probed live: `thingTextureStems()` has the wolf at kind 7 with stem
`pawn/animal/wolf`, `moverParts(7)` returns one slot with that stem and 1536 subframe floats. The
corpus, the kind ids and the DSL wire are all correct, so the cause is downstream of them and the
symptom is not yet explained.

**The one defect found while checking**, which is mine and is the first thing to rule out: P2's
registration loop in `WorldBridge` walks `thingStems` and registers **only** `<base>/<DEFAULT_FACING>`
— i.e. `pawn/animal/wolf/e` — at cell indices `0..15` meaning VARIANTS. But a mover does not resolve
that way:

| path | stem | cell |
|---|---|---|
| cold thing | `<base>/e` | the variant |
| mover | `<base>/<variant>/<facing>`, else `<base>/<facing>` | **none** |

So a mover's south/north stems (`…/wolf/s`, `…/wolf/n`) get **no** registration at all, its
per-variant stems (`…/wolf/3/e`) get none either, and the one key it shares with the cold path
(`…/wolf/e`) is registered with variant-indexed cells the mover never passes — it reads cell 0.
The wolf is the only kind that is both a `thing` in the corpus and rendered as a mover, which is
exactly why it is the kind that broke.

**The mover registration is simply missing** — `MoverLayer` should register from `moverParts()`'s
own `subframes`, against the stems `moverSlotTexture` actually builds, per part slot. That was a P2
item and is not done.

**RESOLVED, and it was not this.** A full stack reset (`rd redeploy --force --run` plus restarting
`rd-master`/`rd-orchestrator`/`rd-worker`/`rd-npc`) settled it: the npc resolves
`def = 0x30010070` — kind **7**, variant 0 — with `speed = 12`, straight from the corpus, and the
freshly-minted mover arrives `kind 7, tics 12, pawn/animal/wolf/e`. The offending entity
(`813694980`) was a **stale row** carrying `kind 1`, which is the tree: kind 1's stem is the conifer,
and the tree authors no speed, so the lookup fell through to `DEFAULT_TICS_PER_TILE = 3`. One stale
kind id, both symptoms.

**Nothing in the DSL work caused it.** Kind ids are assigned in `content/data/things.rd`, which this
stream never touched; only `content/visual/` changed. What the crop did was make it *visible* — an
uncropped kind-1 mover drew a letterboxed conifer at wolf size and read as a vague blob, where a
cropped one is unmistakably a tree.

**Worth keeping as a lesson:** a stale entity presenting as two unrelated-looking bugs (wrong art AND
wrong speed) is a strong signal to check the KIND ID before suspecting the renderer. Both symptoms
resolving to one wrong index is the shape of that failure.

**I11 itself stands and is still unfixed** — the mover registration really is missing, it is simply
not what broke the wolf.

## I12 — CUSTODY (lod-aftermath): the variant-as-cell dialect is retired for folder-variant kinds {#i12}

Written by the lod-aftermath session, 2026-08-03. The I10 registration keyed a cold thing's
per-variant rects as CELLS of the canonical draw stem — but `packCoPack` crops a non-grid stem with
`subframeFor(stem, 0)` alone, and `resolve()` drops the cell on a non-grid entry, so every conifer
ingested the CANONICAL master under VARIANT 0's sapling rect: clipped edges, blown up to fit, nine
variants identical. Fixed in lod-aftermath I3: `thingTexture` now routes `<base>/<v>/e` via a `has`
probe (mirroring `moverSlotTexture`) and `WorldBridge.registerThingSubframes` keys each mastered
variant's rect on ITS OWN stem at `#0`; linked kinds and true `grid` entries keep the per-cell
dialect (probed via the new `resolver.gridOf`). Verify rather than re-implement.

Two findings left in YOUR charter:

1. **`setSpriteScale` is dead-keyed for cold things** (feeds your open F5 compose item): the
   registration in `WorldBridge.refreshStems` keys the BARE base (`biome-thing/default/conifer`),
   while ingest looks up the DRAW stem (`…/e`) — authored `sprite_scale 0.5` has been inert since
   def-frame-anchors P5. Re-keying it will suddenly APPLY those scales (conifers would halve), so it
   belongs with your I9 drawn-box decision, not as a silent fix.
2. **Thing OVERRIDES don't repaint on a routing change** — their raw params aren't cached the way
   ground overrides are, so one painted before the manifest lands keeps the canonical stem until its
   next state update. Rare (manifest races the FIRST paint only) and self-healing, noted for
   completeness.
