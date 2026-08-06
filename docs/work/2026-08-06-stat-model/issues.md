# Issues — stat-model (anticipated inventory)

## I1 — the payload reshapes AGAIN; dev pawns re-mint {#i1}

Third reshape of the pawn payload (i8 → f32+refs → packed 16+16). Same posture as the
interactions stream: entries are NOT read-compatible, the count-guard makes old-shape entries
read as "no row", dev wolves re-mint through the existing npc path. NEED entries LEAVE the
payload entirely for the sub-table ([F2](forks.md#f2)) — the NEED opcode is DELETED, not
retired ([I11](#i11)).

## I2 — bit-identical derivation across worker/npc/wasm {#i2}

Stats, predicates, quantization, and the piecewise needs eval are now computed in THREE
consumers. One implementation in shared/content + codec, consumed everywhere; the f32↔u16
quantize pair is the sharpest edge (rounding must be defined ONCE — round-half-up at write,
exact dequantize). A drift between the npc's availability check and the worker's gate = ghost
refusals. Tests pin the user's combiner examples and the domain ends.

## I3 — the needs sub-table is a SCHEMA change; subscription SQL is strings {#i3}

New table in the pawn shard (shard-tables macro family), new subscription rows for worker,
npc, and client engine. Per [[build-gates-dont-gate]]: subscription SQL is a string — compiles
green, fails live. Every new `SELECT * FROM needs` gets a LIVE check (row observed over the
wire) before its consumer is trusted.

## I4 — piecewise integration across a condition expiry {#i4}

A rate modifier that EXPIRES mid-window means the depletion rate changed at a knowable tic with
no write. The eval must segment: modified rate to the expiry, base rate after. The proof case
is authored deliberately — `quenched` halves thirst depletion — so the drill can watch the
slope change. Watch the interaction with the future-stamped-row guard: segment boundaries use
the same half-window tic math.

## I5 — GAMEPLAY_CATEGORIES is APPEND-ONLY {#i5}

`stat` joins as subtype id 6. The category palette orders subtype ids AND the corpus-position
seed packing — reordering or inserting renumbers every seed ref silently (registry rows would
disagree with seed fallback on a cold client). Append at the end, assert the existing five ids
in a test, comment the law at the definition site.

## I6 — u16 tic seams: `written_tic` and `set_tic` in new places {#i6}

Condition remaining-now math ([F3](forks.md#f3)) and the sub-table's `set_tic` both do
`now − then` on u16 wrapping values. The half-window guard (future-stamped rows read as fresh,
not ancient) must ship in the shared eval for BOTH — it was found live last stream; this stream
inherits it as a requirement, with tests at the seam.

## I7 — golden churn: reshaped sections reviewed line-by-line {#i7}

The dump changes shape substantially (stats registry, leveled traits, predicate affordances,
carrier `interactions`, fixed-point probes). Same discipline as last stream: extend the dump
first, re-bless once, account for EVERY diff line in completed.md. A quantization change hiding
inside a re-bless is exactly what this catches.

## I8 — the needs table must FAN: edge + client engine grow a row type {#i8}

Payload rows fan today; the new table's rows must reach npc and browser. The edge's fan path,
the client engine's row handling, and the wasm surface all grow a needs-row lane. The panel's
`pawnConditions`/`pawnMood`/`pawnNextCrossing` inputs change shape — no straggler may keep
reading needs out of payload ([I11](#i11)).

## I9 — every affordance must be sayable as a stat predicate {#i9}

`drink_water`'s trait-list gate re-expresses as `can_drink` ⇔ `metabolism > 0` with
`biological_lifeform` contributing +1. This is a forcing function (good) and a migration
(work): the interactions-stream corpus, npc `usable_affordance`, and the worker gate all move.
If some future gate genuinely isn't a stat threshold, that's a schema conversation — not a
reason to keep two gating systems.

## I10 — the two-speed window: `speed` field vs `ground_speed` stat {#i10}

Until the input stream rewires movement ([F12](forks.md#f12)), the wolf carries BOTH
`speed = 12` and walks-level-N. Guard: a loader test asserts the wolf's derived `ground_speed`
at its authored level EQUALS its `speed` field, so the values cannot drift apart during the
window; the input stream deletes the field and the test together.

## I11 — no old-shape reader survives {#i11}

Delete-don't-deprecate applied to readers: payload NEED parsing, f32-word entry composers,
`thing_traits`-as-strings availability checks, and the old affordance `requires`/`variants`
fields are DELETED in the same commits that replace them. Acceptance greps are part of the
phase items — a straggler reading the old shape reports zeros silently, which is worse than a
compile error.
