# Forks — spawn authority

## F1 — SPAWN_REQUEST: the user's layout at fixed arity 3; CREATE stays the one mint path {#f1}

The user's wire (plan review), pinned:

    SPAWN_REQUEST [x:16|y:16] [rotation:4|type:4|subtype:12|kind:12] [v0:4|v1:4|…|v7:4]

Three Imm operands, writes nothing (no write set, no claim). Word 1: RAW tile
coordinates — no client-side nibble packing, readable in logs, and the u16 lanes
bound the in-world check (today's world addresses 12 bits/axis; ≥4096 refuses).
Word 2: the def MINUS its variant nibble, shifted down four (`def = (word &
0x0FFFFFFF) << 4 | variant` — one line, no ambiguity), with the freed top nibble
seeding the initial FACING (0–3; 4–15 refused — spawns stop all facing south;
`pack_pawn_data` already carries it). Word 3: the variant vec as NIBBLES, part i
at nibble i — the ≤4-parts law (primitive-graph) means ONE u32 always suffices,
so the verb stays FIXED arity and the event shard/orchestrator frame it
CORPUS-FREE (a variable tail would need a count word whose truth lives in the
corpus — an invitation to disagree). Nonzero nibbles beyond the corpus-declared
part count REFUSE (a lying tail is loud). The worker's arm validates and queues
the real `PROMOTE CREATE …` program worker-side, so `CREATE`'s existing spawn
LEDGER (`(event_reference, index)` idempotency) remains the single mint gate and
replay protection. Rejected: minting directly from the request (duplicates the
ledger); reusing EXECUTE_INTERACTION (no actor, no carrier, no corpus
interaction — a false fit); variable arity (grows a count word if parts ever
exceed 8, not before).

## F2 — refuse, never nudge {#f2}

An invalid request (unknown/non-pawn def, out-of-world or IMPATHABLE position) is a
LOGGED REFUSAL, whole. Nudging to a nearby cell would make the server guess intent
and make refusals invisible; a debug tool wants loud edges. This moves the
lake-mint guard (attack I2 — the stranded mid-lake wolf) into the authority where
it can never be forgotten by a new client. Client pathable-picks stay as
politeness, not protection (I2).

## F3 — the server composes parts, traits, and needs; the TOML parts are the contract {#f3}

The worker already composes TRAIT + need sidecars (`mint_sidecars`); PART entries
now compose there too (`mint_parts`): the kind's `[[thing.part]]` declarations
say HOW MANY variant nibbles the request may carry (the user's "parts declared in
the toml so we know what to expect") — one PART entry per declared slot with the
def's variant nibble substituted from nibble i (validated: a single-part kind
takes nibble 0 into its OWN def variant, the wolf's coat; a two-part kind maps
nibbles 0/1 to body/head; extra nonzero nibbles refuse). The chat handler's
opcode packing and the npc's payload argument are DELETED (delete-don't-
deprecate). The payload wire format is untouched — only WHO writes it changes.

## F4 — CREATE goes worker-only {#f4}

Leaves `CLIENT_VERBS` (the movement-hardening seam): a client CREATE is now
refused at the edge door. The npc and webgl both switch to SPAWN_REQUEST in the
same stream, so nothing legitimate breaks; anything else composing CREATE was
already outside the design.

## F6 — the TYPE nibble routes: one request spawns ANYTHING {#f6}

The user's observation at plan review: the layout "gives us a spawn request for
anything… in theory the ability to spawn meat on the ground, in addition to
spawning pawns" — and tiles "would currently break, but doesn't have to". Adopted
as a ROUTER on word 2's type nibble, each branch landing on the authority path
that ALREADY exists for its type:

| type | path | validation |
|---|---|---|
| `TYPE_PAWN` | the CREATE ledger mint (F1) | in-world + PATHABLE cell, rotation ≤3, part-count nibbles |
| `TYPE_BIOME_THING` | the cold-overlay `SET` (death/forage/drop's composer) | in-world + the named cell EMPTY on the thing layer (no scan — F2's refusal posture) |
| `TYPE_BIOME_TILE` | the tile-layer `SET` (BUILD_WALL's per-tile write) | in-world (a tile spawn REPLACES ground — the build lane) |

The rotation nibble = FACING for pawns; for things/tiles it flows into the SET's
`data` word (the autotile/rotation lanes live there) — 0 today, open not refused.
The variant nibble 0 maps into the thing/tile kind_reference's variant lane. A
side profit: the console drill lanes stop hand-composing raw SETs — the typed,
validated request replaces that hack class.

## F5 — the teleport hunt is evidence-first {#f5}

The bug-sweep I11 discipline: instrument, reproduce, correlate — then fix. Two
probes: (1) SERVER truth — consecutive `entity_state_log` rows per pawn whose
position delta exceeds what one hop stride can carry (the smoking gun for a bad
resolve/write); (2) CLIENT truth — per-frame render-vs-authoritative divergence
beyond the chase cap's explainable envelope (the smoking gun for bad speculation).
Traffic: npc wander + long player trips + mid-trip interrupts + zone crossings
(the resolve/supersession paths). The suspects, ordered: the mid-chord resolve
writing a position from a MISMATCHED chord (it recomputes chords that may differ
from the walking chain's), interrupts re-anchoring from stale bases, event
replays re-arming client speculation (`lastIntentTic` edge cases), and the chase
snap teleporting the RENDER while auth is fine. Conditions land in issues.md with
row dumps; cheap causes get fixed here, structural ones get named successors.
