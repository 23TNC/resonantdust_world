# Forks — needs & moodlets

## F1 — needs are SATISFACTION, depleting toward zero {#f1}

The user's open question, resolved: "I am uncertain if we want to increase thirst or decrease it."

**Chosen: every need is a 0..1 satisfaction that FALLS; bad states are LOW.** The need named
"thirst" stores the hydration satisfaction; the WORD thirst names the need, the moodlets (Thirsty,
Dehydrated) name the states. Rejected: a rising "thirstiness" scalar — semantically closer to the
English word, but it inverts the dialect for exactly one need class. Hunger, rest, bladder and
every future need read naturally as depleting satisfactions (RimWorld's model), and one dialect
means no per-need direction flag in the DSL, no inverted band comparisons in the eval, and no
"which way does this bar go" question ever again. The moodlet layer is what makes this free: the
player never sees the scalar, so its direction is purely an implementation dialect.

## F2 — conditional moodlets are DERIVED; timed moodlets are STORED {#f2}

A band moodlet ("Thirsty while satisfaction < 0.35") is a pure function of
`(need row, tic, corpus)` — deriving it at every observer means ZERO event traffic while a need
drains, no grant/revoke pair racing the band edge, and no possible desync between panel and Brain.
Rejected: storing band grants — every crossing becomes two events and a revocation bug class.
Timed moodlets (post-drink "Quenched", +mood for N tics) CANNOT be derived — they record that an
event happened — so they get a stored lane `(pawn, moodlet_id, grant_tic, expiry_tic)`, built in
P2 and exercised when the action stream grants the first one. Expiry is compared against the tic,
never deleted by a timer.

## F3 — ONE evaluation implementation {#f3}

`needs_eval` lives in a shared rust crate: npc imports it, the client reaches it through
`shared/wasm`. Rejected: a TS mirror of the band math — two derivations of one edge is the drift
class already paid for (subframe-ingest I8's halo, lod-aftermath I3's dialect split). If the
crossing tic ever disagrees between panel and Brain, it must be impossible by construction, not
by review.

## F4 — needs never tick {#f4}

A need row is `(satisfaction, set_tic)`; the DSL `deplete` (TICS full→empty) makes the current
value and every band-crossing tic COMPUTED, not sampled. No per-tic writes, no server need loop,
nothing on the wire while a need drains. Same posture as movement (speed in TICS/TILE) and the
learned tic↔wall estimate — the tic is the clock; observers evaluate lazily. The row is written
only when something happens: mint, a future drink, a forced set in a drill.

## F5 — mood is ONE scalar, for now {#f5}

`mood = clamp(base 0.5 + Σ active moodlet offsets, 0..1)`. Sims-4 emotion CATEGORIES (a moodlet
being "uncomfortable" vs "sad", the dominant-emotion display) are a later lane — the moodlet
layout carries label + offset + duration now and leaves room for a category id without a layout
break. Rejected for this stream: designing the category taxonomy before a single need exists —
that is the spreadsheet-first failure the moodlet abstraction is meant to avoid.

## F6 — needs live on the PAWN SHARD {#f6}

Need rows + stored moodlets are shard lanes written through the normal event path (edge allowlist
→ queue → worker compose → state fan). Rejected: npc-local need state — an npc restart would
reset every wolf's thirst, and the details panel would be reading a different truth than the
Brain. Shards are the root of truth; the npc is just the first writer.
