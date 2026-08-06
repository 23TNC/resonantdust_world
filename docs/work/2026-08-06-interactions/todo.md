# Plan — interactions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the schema, documented before parsed

- [ ] Write `[[trait]]`/`[[interaction]]`/`[[affordance]]` + carrier `affordances` lists into
      `VARIABLES.md § TOML content schema`, with the id-regime rule stated ([I1](issues.md#i1):
      explicit iff the id rides wire/payload). Acceptance: docs-check green; every field P2
      authors has a spelling here first.
- [ ] Document the effect semantics: `satisfy` scaled by magnitude × `unit` ([F3](forks.md#f3))
      with the drink-3 worked example (+77 u8, clamp 255), `grant` via the condition's own
      `duration`, and the reserved `duration = 0` lane ([I9](issues.md#i9)). Acceptance: the
      schema states drink 3's exact u8 delta.

## P1 — the loader (registries + tables)

- [ ] Load the three categories into `Bundle` registries with the explicit-id law enforced
      (duplicate/zero/missing refuse), `deny_unknown_fields` throughout. Acceptance: crate
      tests — a minimal TOML round-trips each category; a duplicate id refuses loudly.
- [ ] Flat consumer tables: per-kind trait sets (things), per-def affordance lists with
      magnitudes (tiles + things), interaction effect params — same accessor posture as
      `thing_needs_table` ([F2](forks.md#f2)/[F6](forks.md#f6)). Acceptance: a crate test reads
      drink's scaled effect through the accessor a consumer will call.
- [ ] Extend the golden dump with the new registries/tables and re-bless (`BLESS_GOLDEN=1`),
      reviewing the diff as added-sections-only ([I8](issues.md#i8)). Acceptance: golden 3/3
      green; the diff shows no byte moved in an existing section.
- [ ] The 2-pass gate (native + wasm32) green with the new surface. Acceptance: `bin/rd`'s
      shared check green both passes.

## P2 — the corpus (the first three defs)

- [ ] Author `content/interactions.toml`: trait `biological_lifeform` (id 1), interaction
      `drink` (id 1, `unit = 0.1`, satisfy thirst, grant quenched, `duration = 0`), affordance
      `drink_water` (id 1, requires biological_lifeform, interaction drink) — comments carrying
      the model, as needs.toml does. Acceptance: loader accepts; `rd content-check` green.
- [ ] Assign the ends: the wolf thing def gains `traits = ["biological_lifeform"]`; the water
      tile def gains `affordances = [{ name = "drink_water", magnitude = 3 }]`. Acceptance: the
      golden re-bless shows exactly the wolf's trait row + water's affordance row.
- [ ] Add the new categories to the client-corpus filter EXPLICITLY as client-served
      ([I10](issues.md#i10)). Acceptance: `/content` lists interactions.toml; biomes still
      withheld.

## P3 — the wire + the worker resolve

- [ ] Add `EXECUTE_INTERACTION = 12` (`&[Write, Imm]` — pawn, affordance id) to
      `shared/codec/action.rs` + the edge verb allowlist; nothing existing moves. Acceptance:
      codec test round-trips; the edge relays the verb from an uplink client.
- [ ] Worker gate: resolve the affordance → trait check via the per-pawn-shaped API
      ([F6](forks.md#f6)) → carrier def on the pawn's tile ∪ 4-neighborhood
      ([I6](issues.md#i6)); refusals log the reason and splice nothing. Acceptance: a
      hand-injected drink off-water logs the refusal; on-water passes the gate.
- [ ] Worker effect: current satisfaction via the shared `needs_eval` at now-tic
      ([I3](issues.md#i3)/[I5](issues.md#i5)), + magnitude×unit clamped to 255, then relay
      `SET_NEED` + `GRANT_CONDITION`(quenched). Acceptance: drill — the row moves by exactly
      +77 from the eval's computed value; quenched appears in the payload with corpus expiry.
- [ ] Wrap-window drill: run one hand-injected drink with `set_tic` across a u16 wrap seam.
      Acceptance: the computed satisfaction matches an offline `needs_eval` computation; no
      stale quench.

## P4 — the npc drinks

- [ ] Bot surface: expose the known-zone tile scan a brain needs to find the nearest
      affordance-carrying tile ([I2](issues.md#i2)). Acceptance: wolves logs the nearest water
      position from its snapshot at a known fixture spot.
- [ ] Wolves brain: on Thirsty/Dehydrated, MOVE_TO the nearest water, `EXECUTE_INTERACTION` on
      arrival, latched once per band crossing; unreached targets log and re-decide, never spin
      ([I2](issues.md#i2)). Acceptance: drill-scaled ([I11](issues.md#i11)) — walk, drink,
      satisfaction jump, Thirsty clears, all in one log arc.
- [ ] The panel shows the arc with ZERO client changes: Thirsty card → walk → Quenched card.
      Acceptance: browser captures of both card states during the live drill.

## P5 — the verdict

- [ ] Docs + memory truth pass: component docs re-pointed where they touch actions/needs, the
      stream memory written, index row updated. Acceptance: `bin/rd docs-check` green.
- [ ] Cold-boot the stack; standing drills (wolf trip, thirst crossing, panel cards) + the
      UNPROMPTED drink arc green together; **the user's eyes close the stream**. Acceptance:
      captures + logs recorded in `completed.md`.
