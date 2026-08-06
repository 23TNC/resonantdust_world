# Issues — interactions

_Seeded at planning (2026-08-06, revised after the user's seven answers): the anticipated
problems, each resolved by a fork, scoped out deliberately, or carried as a live constraint.
New issues found during execution append below._

## I1 — u32 registry ids do NOT fit the payload's 8-bit id lanes {#i1}

The direct consequence of [F1](forks.md#f1): today's payload words pack `need_id:8` and
`condition_id:8`, sized for the explicit ids that just died. A registry id is a full u32
taxonomy tuple. The NEED/CONDITION payload entries must GROW (the payload is a variable-arity
`opcode:16|count:16` command buffer by design — an entry becomes header + def-id word +
sat/tic word), the pawn module's splice composers move with them, and LIVE pawn rows in the old
shape are invalid. Migration = dev-world re-mint: wolves re-initialise their needs through the
npc's existing mint path; no in-place converter is built.

## I2 — the f32 migration touches every satisfaction consumer at once {#i2}

[F3](forks.md#f3)/[F7](forks.md#f7) change both STORAGE (the u8 lane → an f32 payload word) and
DOMAIN (the implicit 0..255 fraction → each need's authored min/max). `needs_eval` (crossing
math, deplete rate, clamping), the corpus bands, the panel's display math, npc logging, and the
golden probes all speak the old scale today. This is ONE migration done everywhere in the same
phase — a consumer left on the 255 scale reads garbage. The golden fixture is the net: its
needs section reshapes entirely and the re-bless diff must be read line-by-line ([I8](#i8)).

## I3 — satisfy is a read-modify-write on a LAZY value {#i3}

Nothing stores current satisfaction; the worker computes it via `needs_eval` at now-tic,
applies the signed delta, writes `(sat′, now)`. A concurrent `SET_NEED` landing between read
and write is clobbered. Benign today — the worker path is the single writer for a pawn's need
words — but it is a CONSTRAINT, not a property: if writers multiply, the resolve must move
inside the pawn module's transaction. Recorded so the shortcut isn't mistaken for safe.

## I4 — the worker executes as instructed; authority is a later refinement {#i4}

Per [F4](forks.md#f4), this turn the affordance (trait gate, availability) is consulted by the
INITIATOR (the npc), and the worker checks only what execution itself needs (the def resolves,
inputs decode, the [F8](forks.md#f8) on-tile carrier holds). The raw `SET_NEED` door (verb 10)
also stays open. Ownership/authorization — who may execute what on which pawn — is a stream of
its own; this one builds the mechanism that stream will harden.

## I5 — u16 tic wrap inside the effect computation {#i5}

The worker's execute does tic arithmetic (`set_tic` deltas, quenched expiry) in the same
wrapping u16 space as `needs_eval`. Rule: ALL tic math goes through the eval's helpers — no
ad-hoc subtraction at the execute site. A P3 drill runs one drink across a wrap seam.

## I6 — event inputs are untyped u32 words {#i6}

The event carries `input_count` + raw words ([F4](forks.md#f4)); meaning comes from the
interaction's declared signature ([F5](forks.md#f5)). A mis-ordered input is silent garbage —
an entity id read as a magnitude. The worker validates count against the signature and
type-checks what it can: an entity input must resolve to a live pawn, a def input must exist in
the registry, and an f32 input must be FINITE (NaN/±inf from the wire poison every downstream
computation — reject, per [F3](forks.md#f3)). A mismatch logs and drops the event, never
half-executes.

## I7 — double-drink refreshes quenched; it does not stack {#i7}

Grants upsert by condition, so drinking twice quickly EXTENDS quenched's expiry rather than
doubling its mood. Chosen behavior (the payload's upsert law), recorded so the "why doesn't it
stack" question has a written answer.

## I8 — golden fixture churn must be READ, not waved through {#i8}

Three new categories, the needs/conditions taxonomy migration, the i8 domain remap, and two def
edits all land in one stream. The gate only guards if each re-bless diff is reviewed: expected
sections added/reshaped, and NO byte moving in a table the phase didn't touch.

## I9 — the client-corpus filter must classify gameplay TOMLs deliberately {#i9}

Server-only content is filtered by what the data IS (content-packages F2). Gameplay defs SHIP:
the client's eval already consumes needs/conditions, and a future UI reads
interactions/affordances. That is a decision recorded at the filter site with the biomes
precedent cited — not a default inherited by whatever category comes next.

## I10 — the real deplete rate is unobservable live {#i10}

Thirst full→empty is 21600 tics (1 h wall). Drills seed near band edges via the existing
`NPC_THIRST` lane; authored corpus numbers stay production values. Every live acceptance in
P3/P4 states its drill seed so observed numbers are reproducible.
