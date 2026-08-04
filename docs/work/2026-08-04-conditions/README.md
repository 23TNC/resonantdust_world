# Conditions — the word changes, and they become cards — 2026-08-04

_Components: [`shared/content`](../../components/shared/), `shared/codec`, `shared/wasm`,
`server/spacetime` (pawn module + edge/worker verbs), [`client/npc`](../../components/client/npc/),
[`client/webgl`](../../components/client/webgl/). Plan in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md); the one open call in [`blockers.md`](blockers.md)._

## The user's call

> "We are going to rename moodlets to feelings. In our details panel we will display feelings as
> cards. We will display the 4 highest priority maximized displayed horizontally along the bottom
> of details with padding between the bottom and left edge as well as padding between the cards.
> They will extend left to right as a pawn acquires more feelings. A minimized feeling will reduce
> its width to a percentage of its width to save horizontal space. When we click on a minimized
> feeling we will maximize all feelings. This will cause feelings to exceed the details width. If
> we cannot exceed a panel's width please let me know and I'll determine another method to handle
> this." — 2026-08-04

> "I've come to understand that moodlets can affect more than feelings, for example needs. So
> instead of renaming to feelings lets rename to conditions." — 2026-08-04

## Two things, in order

**1. The word.** `moodlet` → `condition` — chosen over *feeling* (names one effect of many) and
over *card* (already the renderer's billboard quad, [F0](forks.md#f0)); the **UI element** stays
the card. The pawn HAS conditions, the panel SHOWS cards. Renamed everywhere it is written: the authored corpus
(`content/needs.toml`), the loader types, the ONE eval, the codec payload op and action name, the
pawn module's reducer, the regenerated bindings, the npc, the client, and the authoritative docs
(`VARIABLES.md`, `TABLES.md`, `ACTIONS.md`). Not a UI relabel ([F1](forks.md#f1)) — two vocabularies
for one concept is exactly the drift class this repo has already paid for. **No wire value moves**:
`GRANT_MOODLET = 11` keeps opcode 11 as `GRANT_CONDITION`, `PAYLOAD_OP_MOODLET = 3` keeps 3, ids in
`needs.toml` stay pinned. The rename is a name, and the golden fixture proves the tables are
otherwise identical.

The new word **collides with the old sub-vocabulary**: today a band-derived moodlet is called a
*conditional* moodlet, which becomes "a conditional condition". The two kinds are re-named
**derived** (a band on a need's satisfaction, computed by every observer) and **timed** (a stored
grant with a duration) — [F4](forks.md#f4).

**Why the user changed it, and what the plan must preserve.** A condition is not only a mood
offset; it can act on other pawn state — a need's depletion rate, later a stat. This stream does
**not** build that effect system, but it must not encode "mood offset" as the definition of the
thing: `ConditionParams` keeps `mood` as one named effect field with room beside it, and the docs
say so ([F6](forks.md#f6)). Building the effect table is the successor stream's charter, alongside
the still-pending drink action.

**2. The cards.** The details panel's conditions stop being monospace rows and become a horizontal
strip of cards along the bottom of the panel body: padding from the bottom edge, padding from the
left edge, padding between cards, growing left→right as the pawn acquires more. The **4 highest
priority** are maximized; the rest are minimized to a fraction of a card's width. Clicking a
minimized card maximizes **all** of them.

**Priority is authored** ([F2](forks.md#f2)) — a `priority` integer on each condition def,
defaulting to 0, tie-broken by `|mood|` then `condition_id`. Derive-from-mood cannot express "mild
but urgent", it is not tunable without changing what the player feels, and it stops meaning
anything the moment a condition's effect is a need rather than a mood. The ordering is computed in
the ONE shared eval ([F3](forks.md#f3)), not in TS — same reason the band comparison lives there.

## The strip is a SIBLING of the panel — [B1](blockers.md#b1) → [F7](forks.md#f7)

`PANEL_CSS.overflow = "hidden"` on the panel root ([DomPanelStyles.ts:30](../../../client/webgl/src/ui/dom/DomPanelStyles.ts))
clips every child at the panel's border; the body is `overflow: auto`
([DomPanelStyles.ts:177](../../../client/webgl/src/ui/dom/DomPanelStyles.ts)), so a card strip
*inside* the panel would scroll rather than spill. Nothing parented to the panel can paint outside
it — raised as [B1](blockers.md#b1), and the user resolved it: **the strip is a sibling of the
details panel, so it can actually draw past it.**

So the strip is a `position: fixed` element appended to the panel's HOST, anchored to the panel's
bottom-left off the panel's existing `rectChange` event, in **both** states — collapsed and
expanded ([F7](forks.md#f7)): one layout, one code path, no reparent at the moment of the click.
It looks like it sits inside the panel's bottom edge and simply keeps going when it outgrows it.
The clamp moves outward with it — the strip scrolls internally only once it reaches the **viewport**
edge, not the panel edge.

## Non-goals

Mood is not redesigned — it stays the one clamped scalar. The effect system beyond mood is named
and shaped for, not built. No need bars, this stream or ever. Card ART (icons, per-condition
colours, the Sims emotion categories) is out of scope: a card is label + mood offset + timer,
styled from panel chrome. The action system that grants timed conditions is still the successor of
[`2026-08-03-needs-moodlets`](../2026-08-03-needs-moodlets/README.md).

## Exit

`grep -rin moodlet` over the tree returns only historical work-stream prose; the golden fixture
re-blesses to byte-identical tables under the new names; a selected wolf shows its conditions as
cards along the bottom of details, four maximized, the rest minimized, and clicking a minimized one
maximizes all under whichever overflow method the user picks. The user's eyes close the stream.
