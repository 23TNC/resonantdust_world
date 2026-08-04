# Forks — conditions

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
