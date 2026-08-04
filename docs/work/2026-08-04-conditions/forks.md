# Forks — conditions

## F0 — the word is "condition"; the UI element is the "card" {#f0}

_2026-08-04. The user landed here in three steps: moodlet → feeling → condition → (considered)
card. Recorded in full so nobody re-opens it._

**feeling** was rejected by the user themselves: "moodlets can affect more than feelings, for
example needs" — the word names one effect and the thing has an open set of them ([F6](#f6)).

**card** was proposed to avoid a conflict with "conditions", on the belief that the game has no
cards yet. It does: a **card** is the renderer's billboard quad — the vertical plane every sprite
is drawn on — in `game/viewport/worldTilt.ts`, `game/viewport/records.ts` and `game/world/MoverLayer.ts`,
and it is authoritative vocabulary in [`VARIABLES.md`](../../VARIABLES.md) ("the caster's card top",
"the n/s perpendicular caster card"). `debug/index.ts` also registers a `"cards"` channel and
`game/layout/LayoutNode.ts` reserves the word for a future UI system. Taking it would mean renaming
the billboard card across the whole render path first — a larger rename than this stream's.

**Chosen (user, 2026-08-04)**: **condition**. Its only code collision is one biome-rule predicate
([`loader.rs:575`](../../../shared/content/src/loader.rs), a local `condition` on a biome rule — a
different domain, not an exported name), plus the "conditional moodlet" sub-vocabulary that
[F4](#f4) retires to derived/timed anyway.

**effect** was the runner-up: zero identifier collisions today and a good fit for [F6](#f6)'s open
effect set. Rejected because a renderer with lighting and shaders will plausibly want "effect" for
VFX, trading a present collision for a likely future one.

The **UI element keeps the name "card"** — the details panel draws each condition as a card. That
is a DOM widget in `game/panels/details/`, far from the render path, and it is the user's own word
for the visual. The pawn HAS conditions; the panel SHOWS cards.

## F1 — the rename goes all the way down, not just to the label {#f1}

_2026-08-04._ "Rename moodlets to conditions" could be satisfied by changing one display string in
`content/needs.toml` and stopping. It isn't.

**Chosen**: rename the identifier in every layer — corpus keys, loader types, the shared eval, the
codec payload op and action constant, the pawn module's reducer, the regenerated bindings, npc,
client, and the three authoritative docs. Wire VALUES do not move (opcode 11, payload op 3, ids
1..3), so the change is provably name-only and the golden fixture is the proof.

Rejected: **UI-only relabel** — the repo would then hold two words for one concept, which is the
drift class already paid for twice ([subframe-ingest I8](../2026-08-02-subframe-ingest/issues.md),
[lod-aftermath I3](../2026-08-02-lod-aftermath/issues.md)); a reader hitting `grant_moodlet` in the
module and "Condition" in the panel has to derive that they are the same thing every time.
**Renaming the wire values too** — nothing is gained and every stored pawn payload would need
migration; ids are stored data and never renumber ([needs-moodlets F1](../2026-08-03-needs-moodlets/forks.md#f1)).

## F2 — priority is authored, not derived {#f2}

_2026-08-04._ The panel shows the 4 highest-priority conditions maximized, so "priority" has to
mean something. Options were: sort by `|mood|`, sort by recency of acquisition, or author it.

**Chosen**: an optional `priority` integer on each condition def (default `0`), sorted desc, tie-
broken by `|mood|` desc then `condition_id` asc. Priority is a tuning knob, and tuning knobs in
this repo are corpus data — the same posture as `deplete`, speed in TICS/TILE, and every other
number a designer wants to move without a rebuild.

Rejected: **derive from `|mood|`** — cannot express "mild but urgent" (a small mood offset the
player must notice), and it stops meaning anything the moment a condition's effect is a need rather
than a mood ([F6](#f6)) — a condition with no mood at all would sort last forever. **Recency** —
the card order would reshuffle as timers land, so the eye can never learn where a condition sits;
also unstable across observers, which breaks the "every observer computes the same thing" rule.

## F3 — the sort lives in the ONE eval {#f3}

_2026-08-04._ Ordering could be done in TS in the panel, in one line.

**Chosen**: `active_conditions` returns its result already ordered, and the wasm stride widens to
carry `priority` so the client can render but never re-decide. Every observer — panel, npc brain,
any future tooltip — sees the same order.

Rejected: **sorting in the panel** — it is a second implementation of a corpus rule in a second
language, which is exactly what [needs-moodlets F3](../2026-08-03-needs-moodlets/forks.md#f3) ruled
against for the band comparison. The npc will want priority ordering for decisions too, and it does
not run TS.

## F4 — "conditional" retires; the kinds are DERIVED and TIMED {#f4}

_2026-08-04._ The old vocabulary called a band-derived moodlet a *conditional* moodlet. Under the
new word that is "a conditional condition", and `duration == 0` would be documented with a term
that now names the whole category.

**Chosen**: the two kinds are **derived** (a band `lo <= sat < hi` on a need's satisfaction,
computed by every observer from `(need row, tic, corpus)`, never granted) and **timed** (a stored
grant `(condition_id, grant_tic)` alive for `duration` tics). `duration == 0` means DERIVED.

Rejected: **keeping "conditional"** — self-referential and unreadable. **"band" vs "grant"** as the
pair — accurate for the mechanism but not for the concept; a future derived condition might key off
something other than a need band, and the word should survive that.

## F5 — a click toggles, so there is a way back {#f5}

_2026-08-04._ The user specified one direction: clicking a minimized card maximizes all. Nothing
says how to return to the 4-maximized default, and a state with no exit is a trap.

**Chosen**: the expanded flag is a toggle — a click on any minimized card sets it, a click on any
card while expanded clears it. It is panel state (persisted with the panel's other prefs), not
selection state, so it survives selecting a different pawn.

Rejected: **auto-collapse on selection change** — the user set expanded deliberately; re-collapsing
it behind their back means re-clicking for every pawn. **A separate collapse control** — a chevron
or a close button is more chrome than the strip can carry at 12px, and the card is already the
click target.

## F6 — `mood` is one effect, not the definition {#f6}

_2026-08-04._ The user's correction — "conditions can affect more than feelings, for example needs"
— is a statement about the model's future, and this stream must not encode against it.

**Chosen**: `ConditionParams` keeps `mood` as a named effect field with room beside it, and
`VARIABLES.md` says a condition's effects are an OPEN set of which `mood` is the first. The effect
table (need-rate modifiers, later stats) is the successor stream's charter, alongside the still-
pending drink action.

Rejected: **building the effect system now** — the user asked for a rename and a card layout, and
an effect table is a wire + eval + npc-decision change that deserves its own stream and its own
exit. **Leaving the docs saying "a condition is a label + a mood offset"** — that is the sentence
the next session would build against, and it is now known to be wrong.

## F7 — the card strip is a SIBLING of the details panel, in both states {#f7}

_2026-08-04. Resolves [B1](blockers.md#b1) — the user's call: "Please make the cards a sibling of
the details panel so that it can actually draw past the details."_

A panel root is `overflow: hidden`, so nothing parented inside `DetailsPanel` can paint past its
border. The strip therefore lives in its own `position: fixed` element appended to the panel's
HOST (`host.appendChild(this.panel)` at [DomPanel.ts:1233](../../../client/webgl/src/ui/dom/DomPanel.ts)),
making it a sibling — outside the clip, free to extend right across the world.

**Chosen**: the strip lives in the overlay in **both** states — collapsed (4 maximized + the
minimized remainder) and expanded (all maximized). It is positioned against the panel's bottom-left
with the authored padding, so it *looks* like it is inside the panel's bottom edge and simply keeps
going when it outgrows it. The panel already fires `rectChange` on construction, drag, resize,
window resize, anchor flip and minimize toggle — the overlay re-anchors off that one event, and
mirrors the panel's visibility (close / minimize / hide / nothing-selected all hide it).

Rejected: **strip inside the body until expanded, then reparent to an overlay** — two layouts, two
code paths and a visible jump at the exact moment the user clicks; the collapsed strip would also
inherit the body's scroll, so it would slide away from the bottom edge as the text rows scroll.
**Overlay only when it overflows** — same reparent, just triggered by a measurement instead of a
click, which makes the jump intermittent and therefore harder to see in testing.

Consequences carried into P6: the overlay needs a z-index above the panel band, must not swallow
world input outside its own bounds, must clamp at the **viewport** edge (scroll inside the overlay
past that — [B1](blockers.md#b1) method #1 as the inner fallback), and must be torn down with the
panel.

## F9 — the strip slides before it scrolls {#f9}

_2026-08-04, resolved during P6._ [B1](blockers.md#b1) chose the sibling overlay with in-strip
scrolling as the inner fallback. Implementing it exposed a case the blocker's table did not
separate: a panel **near the right edge of the screen**. Anchoring the strip at `panelLeft + 8`
would push it off-screen, and falling straight to "scroll inside the strip" would mean a user with
a right-docked details panel scrolls to see cards that would have fit on screen perfectly well.

**Chosen**: two distinct cases. If the strip FITS the viewport, slide its origin left until it does
— `left = clamp(panelLeft + PAD_LEFT, EDGE_MARGIN, viewportW − EDGE_MARGIN − naturalWidth)`. Only
when the strip is wider than the **screen** does it become a scroller. Measured: a bottom-right
panel at `left 1504` gets a strip at `left 1344` (slid 168 px), right edge 1854 inside a 1862 px
viewport, no scrolling, all six cards visible.

The scroll branch takes `pointer-events: auto`, which the fitting branch does not — a scroller that
does not receive pointer events cannot be scrolled. The cost is real but narrow: at that width the
strip already spans the viewport, so the world it shadows is a 42 px band at the very bottom.

Rejected: **always scroll when the natural origin overflows** — makes a right-docked panel feel
broken for no benefit. **Flip the strip to grow leftwards from the panel's right edge** — the strip
would then change direction depending on where the panel sits, so the first card moves under the
user; the sort order is the one thing that must stay put.

Width is computed **arithmetically** from the card counts, not measured off the DOM, so `reflow`
never forces a synchronous layout — it runs on every drag frame.

## F8 — the conditions drill lives in the npc brain {#f8}

_2026-08-04, resolved during P1._ Two of this stream's acceptance criteria are unreachable on the
authored corpus: `GRANT_CONDITION` has **no caller** (the drink action is the successor stream), and
thirst depletes over 21600 tics — an hour of wall clock before the first band crossing. P4 also
asks for "6 conditions forced onto a wolf".

**Chosen**: two env drills on the `Wolves` brain. `NPC_THIRST=<0..=255>` makes the one-shot mint
write that satisfaction instead of full, so the wolf starts inside whichever band you want.
`NPC_GRANT=<ids>` queues one `GRANT_CONDITION` per id after the mint. Both are unset by default —
the authored behaviour is untouched — and both go through the ordinary client verb path, so what
they exercise is the real chain, not a test double.

Rejected: **editing `content/needs.toml`'s `deplete` for a test run** — the corpus is the game's
data, a test must not require mutating it, and the golden fixture would fail for the duration.
**A one-off script that pokes the reducer directly** — it would bypass the edge allowlist and the
worker relay, i.e. exactly the two hops this stream renamed and most needs proving.
**Waiting the hour** — not a test loop.
