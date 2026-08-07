# Forks — emotions (plan-time decisions; each is mine unless the user vetoes)

## F1 — u4 declaration-order identity; `fine` REQUIRED at index 0; not registry-numbered {#f1}

Sixteen `[[emotion]]` blocks; the loader enforces ≤16 (the u4 bound), `fine` first
(index 0 IS the "alters nothing" default), and unique names. NOT in the gameplay
registry: emotions never cross the wire — every consumer evaluates from its corpus, so
a u32 definition_reference would be dead weight and the u8 packing (`emotion:4 |
magnitude:4`, the user's layout) is the canonical compact form.
**Rejected**: registry numbering (nothing routes or stores emotion refs); a fixed enum
in code (the corpus owns look AND membership — a 17th emotion should be a TOML error,
not a recompile).

## F2 — modifiers on conditions (scalars) and traits (per-level arrays) {#f2}

`emotions = [{ emotion = "happy", magnitude = 6 }]` on conditions;
`emotions = [{ emotion = "playful", magnitude = [1, 2, 3] }]` on traits (index =
level − 1, the stats pattern). Magnitude bounds 1..15 at load — +0 is authored by
ABSENCE (the user: "+0 will be omitted"), +15 is the u4 ceiling.
**Rejected**: needs carrying emotion modifiers directly (bands already express
need-state as CONDITIONS — thirsty/dehydrated author the emotions).

## F3 — active = argmax of sums; empty → fine; ties → lowest index {#f3}

ONE `emotion_eval::active_emotion(bundle, trait_rows, active_conditions) → (u4, [u16; 16])`
in shared/content beside the other evals — sums per emotion over traits + ACTIVE
conditions, argmax wins. No contributions → `fine` (index 0). A tie takes the LOWEST
index — deterministic everywhere, and fine-first ordering biases ties toward calm.
**Rejected**: most-recent-wins (needs stored recency the rows don't carry); random/
oscillating ties (two observers would disagree — the one-eval law exists to prevent
exactly that).

## F4 — mood retires ENTIRELY (USER, 2026-08-07) {#f4}

Confirmed at plan time: "mood is being replaced entirely with emotion modifiers…
I am uncertain why we would require mood for conditions to function" — and they don't:
mood was only ever a display scalar and a sort input. Retired: the conditions' `mood`
field (corpus + loader), `pawnMood`, the `pawnConditions` stride's mood lane, the
panel's `mood N%` row (the active emotion's wash + tooltip carry the affect surface),
and the card sort's `|mood|` tie-break — which becomes SUMMED EMOTION MAGNITUDE desc
(a condition's total intensity), keeping the total order (priority desc → Σ magnitude
desc → id asc) computed once in the shared eval. The npc's decision order follows
automatically (it consumes the sorted list).

## F5 — the pie is a CSS conic-gradient on a square card {#f5}

Cards become fixed-size SQUARES, background = `conic-gradient` with one slice per
modifier, proportional to magnitude, slices in emotion-index order from 12 o'clock;
one modifier = solid color; none = solid `fine` gray. No SVG needed; the existing
ConditionCards strip keeps its geometry/positioning machinery (now cleared of the
intent strip per intent-queue-ui).
**Rejected**: per-slice SVG paths (conic-gradient is exact and cheap); recharts-style
donut (the card IS the chart).

## F6 — the tooltip is the IntentStrip pattern, showing every property {#f6}

One shared fixed-position tooltip div: the condition's LABEL, one `+N <Emotion>` line
per modifier (omit +0 by construction, always render `+`), `priority`, `remaining`
tics for a TIMED grant, and the need modifiers (`rate ×0.5 thirst` style). Nothing
renders as card text anymore — the tooltip is the whole read surface (the user's call).

## F7 — the panel wash is alpha-dimmed {#f7}

The details panel BODY background = the active emotion's color at low alpha over the
panel's dark base (literal first-pass ask, kept readable for the monospace rows). The
swatch updates on the same cadence the panel already refreshes (500 ms + fan renders).

## F8 — I author the 12 unspecified colors; the corpus owns tuning {#f8}

User-fixed: sad blue, angry red, embarrassed yellow, fine gray. Authored by me,
distinct-hue, dark-theme-legible: happy gold-orange, confident purple, tense
red-brown, bored slate, confused teal, scared violet, motivated green, restless
amber, focused cyan, affectionate pink, uncomfortable olive, playful magenta. One
TOML line each to retune.
