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

## F7 — need rows are PAYLOAD OPCODES, not new tables {#f7}

The plan's P2 wording ("a pawn NEED lane via the `*_tables!` family") turned out to be a plan bug:
the pawn module's PAYLOAD sidecar (human-pawns P0/F5) is the DOCUMENTED home for exactly this —
its header reads "a pawn's open-ended state (parts, and later inventory/stats/**needs**/…)", and
`shared/codec/payload.rs` promises "the stream grows new opcodes (inventory, stats, needs, …)
without breaking old readers; opcode ids are APPEND-ONLY". Building a parallel table pair would
re-plumb a pipe that already runs end-to-end: `payload` rows are zone-keyed, slaved to the
entity's claim, fanned by the edge (`on_insert`/`on_update` + the zone subscription), decoded by
`client/core` — the PART opcode proves every stage.

**Chosen**: `NEED` (opcode 2, word `need_id:8 | satisfaction:8 | set_tic:16`) and `MOODLET`
(opcode 3, word `moodlet_id:8 | _:8 | grant_tic:16` — expiry DERIVED from the corpus duration,
never stored) entries in the pawn payload; `SET_NEED` / `GRANT_MOODLET` verbs composed by the
pawn module itself (the reducer reads + splices the current payload — the worker relays, so no
new worker read-set). Rejected: the literal new-table plan — new subscription strings, new fan
plumbing, a second zone-follow, all duplicating the sidecar. The P2 acceptance criteria carry
over unchanged (TABLES.md documents the opcodes; the hand event still lands + fans).

## F6 — needs live on the PAWN SHARD {#f6}

Need rows + stored moodlets are shard lanes written through the normal event path (edge allowlist
→ queue → worker compose → state fan). Rejected: npc-local need state — an npc restart would
reset every wolf's thirst, and the details panel would be reading a different truth than the
Brain. Shards are the root of truth; the npc is just the first writer.
