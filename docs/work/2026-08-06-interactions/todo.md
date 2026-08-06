# Plan — interactions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the schema, documented before parsed

- [ ] Write the gameplay taxonomy into `VARIABLES.md § TOML content schema`: `type = "gameplay"`,
      subtypes trait/interaction/affordance/need/condition, kind, variant `default`; registry
      numbers the tuple, NO authored ids — the needs/conditions blocks rewritten off `id = N`
      ([F1](forks.md#f1)). Acceptance: every field P2 authors has a spelling here first.
- [ ] Document the value domain: satisfaction = −128-biased i8 (full 127, empty 0, negative =
      deficit, [F3](forks.md#f3)/[F7](forks.md#f7)); bands/deplete/magnitudes in RAW units; the
      NEED/CONDITION payload entries grown to u32 def ids ([I1](issues.md#i1)). Acceptance: the
      schema states drink 3's exact stored delta (+3) and the new entry layouts.
- [ ] Document the `EXECUTE_INTERACTION` event: `[op, interaction_id, version, input_count,
      inputs…]`, inputs bias-encoded i32, the input signature declared by the interaction def
      ([F4](forks.md#f4)/[F5](forks.md#f5)). Acceptance: layout in VARIABLES before any codec
      change; docs-check green.

## P1 — the loader + the registry

- [ ] Loader: parse `[[trait]]`/`[[interaction]]`/`[[affordance]]` by taxonomy with
      `deny_unknown_fields`; needs/conditions migrate off explicit ids to taxonomy; duplicate
      `(taxonomy, version)` refuses. Acceptance: crate tests — round-trip per category; a
      duplicate tuple refuses loudly.
- [ ] `needs_eval` moves to the i8 domain: raw-unit bands, deplete = TICS for 127→0 continuing
      below zero, biased-storage helpers used by every caller ([I2](issues.md#i2)). Acceptance:
      crate tests + translated probes green; no fraction math survives grep.
- [ ] Registry: the index module numbers `gameplay` tuples like things ([F1](forks.md#f1)).
      Acceptance: registry rows exist for the P2 defs; a consumer resolves
      `gameplay/interaction/drink/default` → its u32.
- [ ] Flat consumer tables: per-kind trait sets, per-def affordance lists with magnitudes,
      interaction signatures + effects ([F2](forks.md#f2)/[F5](forks.md#f5)/[F6](forks.md#f6)).
      Acceptance: a crate test reads drink's signature and effect through the consumer accessor.
- [ ] Extend the golden dump (new registries/tables; the reshaped needs section) and re-bless,
      reviewing the diff section-by-section ([I8](issues.md#i8)). Acceptance: golden 3/3 green;
      the review recorded in completed.md.
- [ ] The 2-pass gate (native + wasm32) green with the new surface. Acceptance: `bin/rd`'s
      shared check green both passes.

## P2 — the corpus (the first three defs)

- [ ] Author `content/interactions.toml`: trait `biological_lifeform`; interaction `drink`
      (inputs pawn/need/amount, satisfy by signed amount, grant `quenched`, `duration = 0`
      reserved); affordance `drink_water` (requires the trait, interaction drink, variants
      `["default"]`) — comments carrying the model. Acceptance: loads; `rd content-check` green.
- [ ] Re-author `content/needs.toml` to the gameplay taxonomy (no ids) and assign the ends: wolf
      `traits = ["biological_lifeform"]`, water tile `affordances = [{ name = "drink_water",
      magnitude = 3 }]`, bands in raw i8 units. Acceptance: the golden re-bless shows exactly
      these rows.
- [ ] Classify the gameplay TOMLs as client-served at the filter site, biomes precedent cited
      ([I9](issues.md#i9)). Acceptance: `/content` lists interactions.toml; biomes still
      withheld.

## P3 — the payload, the event, the worker

- [ ] Codec + pawn module: NEED/CONDITION payload entries grow to carry u32 def ids + biased i8
      satisfaction; splice composers follow; dev wolves re-mint through the npc's existing path
      ([I1](issues.md#i1)). Acceptance: a fresh wolf's payload holds registry ids; no old-shape
      rows remain live.
- [ ] Codec + edge: `EXECUTE_INTERACTION` as a variable-arity event (the CREATE precedent)
      through the edge queue; nothing existing moves. Acceptance: codec round-trip test; the
      edge relays the event from an uplink client.
- [ ] Worker executes: resolve the def (corpus + registry manifest), validate count + decode
      biased inputs ([I6](issues.md#i6)), check the pawn stands ON a carrier tile
      ([F8](forks.md#f8)), then `needs_eval` read-modify-write + `SET_NEED` +
      `GRANT_CONDITION`(quenched) ([I3](issues.md#i3)). Acceptance: a hand-injected drink moves
      satisfaction by exactly +3; off-water logs a refusal and splices nothing.
- [ ] Wrap-window drill: one hand-injected drink with `set_tic` across a u16 wrap seam
      ([I5](issues.md#i5)). Acceptance: the result matches an offline `needs_eval` computation;
      no stale quench.

## P4 — the npc drinks

- [ ] Bot surface: expose the known-zone tile scan a brain needs to find the nearest
      affordance-carrying tile. Acceptance: wolves logs the nearest water position from its
      snapshot at a known fixture spot.
- [ ] Wolves brain: affordance availability check ([F6](forks.md#f6)), then on
      Thirsty/Dehydrated MOVE_TO the nearest water and issue `EXECUTE_INTERACTION` on arrival,
      latched once per band crossing. Acceptance: drill-scaled ([I10](issues.md#i10)) — walk,
      drink, +3, Thirsty clears, one log arc.
- [ ] The panel shows the arc with ZERO client changes beyond the shared eval: Thirsty card →
      walk → Quenched card. Acceptance: browser captures of both card states during the drill.

## P5 — the verdict

- [ ] Docs + memory truth pass: component docs re-pointed where they touch needs/actions, the
      needs-moodlets memory amended (i8 domain, registry ids), the stream memory written.
      Acceptance: `bin/rd docs-check` green.
- [ ] Cold-boot the stack; standing drills (wolf trip, thirst crossing, panel cards) + the
      UNPROMPTED drink arc green together; **the user's eyes close the stream**. Acceptance:
      captures + logs recorded in `completed.md`.
