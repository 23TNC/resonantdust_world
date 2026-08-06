# Plan — interactions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the schema, documented before parsed

- [x] Write the gameplay taxonomy into `VARIABLES.md § TOML content schema`: `type = "gameplay"`,
      subtypes trait/interaction/affordance/need/condition, kind, variant `default`; registry
      numbers the tuple, NO authored ids — the needs/conditions blocks rewritten off `id = N`
      ([F1](forks.md#f1)). Acceptance: every field P2 authors has a spelling here first. →
      landed WITH [F9](forks.md#f9): the taxonomy DERIVES from the category table (authoring it
      twice is a drift lane); also fixed the stale `id = 7` wolf sample (pre-registry leftover).
- [x] Document the value domain: satisfaction and magnitudes are f32; each need authors
      `min`/`max` + sign treatment ([F3](forks.md#f3)/[F7](forks.md#f7)); the NEED/CONDITION
      payload entries grown to u32 def ids + an f32 satisfaction word ([I1](issues.md#i1)).
      Acceptance: the schema states drink 3's exact effect (+3.0, clamped to thirst's authored
      max) and the new entry layouts. → VARIABLES worked example (`clamp(sat + 3.0, 0, 100)`) +
      TABLES §payload rows NEED(count 3)/CONDITION(count 2) + the not-read-compatible re-mint note.
- [x] Document the `EXECUTE_INTERACTION` event: `[op, interaction_id, version, input_count,
      inputs…]`, value inputs as f32 BIT PATTERNS in the u32 lanes, the input signature declared
      by the interaction def ([F4](forks.md#f4)/[F5](forks.md#f5)). Acceptance: layout in
      VARIABLES before any codec change; docs-check green. → landed in ACTIONS.md (the verb
      table's authoritative home), op 12, as a BUILD_WALL-pattern verb: writes NOTHING itself,
      the worker queues PROMOTE SET_NEED/GRANT_CONDITION — so grouping never routes untyped
      input words. SET_NEED/GRANT_CONDITION rows amended to u32 def refs + f32 bits.

## P1 — the loader + the registry

- [x] Loader: parse `[[trait]]`/`[[interaction]]`/`[[affordance]]` by taxonomy with
      `deny_unknown_fields`; needs/conditions migrate off explicit ids to taxonomy; duplicate
      `(taxonomy, version)` refuses. Acceptance: crate tests — round-trip per category; a
      duplicate tuple refuses loudly. → 26/26 lib tests; the taxonomy DERIVES per F9, so the
      aliasing rule is name-uniqueness per category (`defined twice` refusal); a need still
      authoring `id` refuses via deny_unknown_fields; F5 `@ref`s resolve to input indices at
      load, dangling ones refuse.
- [x] `needs_eval` moves to f32 + per-need authored domains: clamp to `min`/`max`, deplete =
      TICS for the max→min traverse, bands in the need's own units ([I2](issues.md#i2)).
      Acceptance: crate tests + translated probes green; no 255-scale math survives grep. →
      rows are `(u32 ref, f32, u16)`; `satisfaction_at` takes `&NeedParams` and clamps to the
      domain; the never-crossed guard generalised `<= 0` → `<= min`; a new test drives the
      0..100 domain incl. the deficit clamp; `/255` gone from the crate.
- [x] Registry: the index module numbers `gameplay` tuples like things ([F1](forks.md#f1)).
      Acceptance: registry rows exist for the P2 defs; a consumer resolves
      `gameplay/interaction/drink/default` → its u32. → the MASTER allocates (the index only
      records, I8 posture): `TYPE_GAMEPLAY = 8` + a code-owned category palette in codec, the
      five category loops in `allocations()`; LIVE: master seeded 189, `/definitions` shows
      the 7 gameplay rows, `drink` = `0x80040010` — equal to the loader's SEED by test.
- [x] Flat consumer tables: per-kind trait sets, per-def affordance lists with magnitudes,
      interaction signatures + effects ([F2](forks.md#f2)/[F5](forks.md#f5)/[F6](forks.md#f6)).
      Acceptance: a crate test reads drink's signature and effect through the consumer
      accessor. → `thing_traits`/`tile_affordances`/`thing_affordances`/`interaction_params`
      (+`_by_ref`) + the F6 gate `affordance_available(name, &traits)`; `thing_needs`
      returns u32 refs and the stride-8 table carries them; sim fingerprints extended.
- [x] Extend the golden dump (new registries/tables; the reshaped needs section) and re-bless,
      reviewing the diff section-by-section ([I8](issues.md#i8)). Acceptance: golden 3/3 green;
      the review recorded in completed.md. → 48 insertions / 7 deletions, every line accounted
      for (the wolf's table slot → `0x80010010`, the 0..100 bands, the added sections, probe
      refs `0x8002_00{1,2,3}0`, and mid's 3262→3241 = the probe's own quantization change).
- [x] The 2-pass gate (native + wasm32) green with the new surface. Acceptance: `bin/rd`'s
      shared check green both passes. → native = codec 64 + content 26+2 golden in docker;
      wasm32 = `rd build shared` rebuilt the pkg bundle clean (with the new
      `conditionLabelOf`/`withGameplayRegistry` surface).

## P2 — the corpus (the first three defs)

- [x] Author `content/interactions.toml`: trait `biological_lifeform`; interaction `drink`
      (inputs pawn/need/amount, satisfy by signed amount, grant `quenched`, `duration = 0`
      reserved); affordance `drink_water` (requires the trait, interaction drink, variants
      `["default"]`) — comments carrying the model. Acceptance: loads; `rd content-check`
      green. → authored with the three-entity model + operand rules in comments; golden
      dumps its params verbatim; content-check clean (6 corpus files).
- [x] Re-author `content/needs.toml` to the gameplay taxonomy (no ids) and assign the ends: wolf
      `traits = ["biological_lifeform"]`, water tile `affordances = [{ name = "drink_water",
      magnitude = 3 }]`, thirst on its authored `0..100` domain with bands in those units
      ([F7](forks.md#f7)). Acceptance: the golden re-bless shows exactly these rows. → fixture
      rows: `water [("drink_water", 3.0)]`, `wolf traits=["biological_lifeform"]`, bands 10/35
      and 0/10; every other def's rows empty, byte-checked.
- [x] Classify the gameplay TOMLs as client-served at the filter site, biomes precedent cited
      ([I9](issues.md#i9)). Acceptance: `/content` lists interactions.toml; biomes still
      withheld. → the filter is BLOCK-based (content-packages F2 strips `[[biome]]` blocks by
      what the data IS), so gameplay categories ship by construction — verified LIVE:
      `/content` = interactions/materials/needs/subtypes/things/tiles, no biomes.

## P3 — the payload, the event, the worker

- [x] Codec + pawn module: NEED/CONDITION payload entries grow to carry u32 def ids + an f32
      satisfaction word; splice composers follow; dev wolves re-mint through the npc's existing
      path ([I1](issues.md#i1)). Acceptance: a fresh wolf's payload holds registry ids + f32
      satisfaction; no old-shape rows remain live. → the reducer SIGNATURES never moved (u32
      args already), only the composed entries; the module republish wiped dev rows and fresh
      wolves minted `[NEED 0x80010010 f32 tic][CONDITION 0x80020030 tic]` — read live via sql.
- [x] Codec + edge: `EXECUTE_INTERACTION` as a variable-arity event (the CREATE precedent)
      through the edge queue; nothing existing moves. Acceptance: codec round-trip test; the
      edge relays the event from an uplink client. → op 12 frames like CREATE (`count` third),
      contributes NOTHING to the sets/routes (untyped inputs — the queued verbs carry the
      writes); framing test green; LIVE: npc→edge→queue→orchestrator (entities=0)→worker.
- [x] Worker executes: resolve the def (corpus + registry manifest), validate count + decode
      f32 inputs rejecting non-finite ([I6](issues.md#i6)), check the pawn stands ON a carrier
      tile ([F8](forks.md#f8)), then `needs_eval` read-modify-write + `SET_NEED` +
      `GRANT_CONDITION`(quenched) ([I3](issues.md#i3)). Acceptance: a hand-injected drink moves
      satisfaction by exactly +3.0; off-water logs a refusal and splices nothing. → BOTH drills
      green: `NPC_INTERACT` off-water → `interaction dropped … (F8: on-tile only)`, zero
      splices; on-water sips exact (`from=11.5648… to=14.5648…`). Found + fixed en route: the
      `queue_at(t+1)` TIC_GAP silent loss ([I11](issues.md#i11), BUILD_WALL had it too) and the
      future-stamped-row phantom (eval half-window guard + test).
- [x] Wrap-window drill: one hand-injected drink with `set_tic` across a u16 wrap seam
      ([I5](issues.md#i5)). Acceptance: the result matches an offline `needs_eval` computation;
      no stale quench. → ran as [D1](deviations.md#d1): `set_tic` is reducer-stamped, not
      injectable — the seam is pinned by the eval's unit tests instead, and the worker's arm
      verifiably contains NO tic math outside the eval fns + `tic_add` (the I5 rule).

## P4 — the npc drinks

- [x] Bot surface: expose the known-zone tile scan a brain needs to find the nearest
      affordance-carrying tile. Acceptance: wolves logs the nearest water position from its
      snapshot at a known fixture spot. → `Bot::nearest_tile`/`tile_kind_at` over the
      `ColdTiles` baseline ⊕ `ColdState` overlays, MERGED cell-wise (a zone streams one row
      PER BIOME — seen live, zone 99 = 3 rows); logged `water=(102,68) from=(106,60)`.
- [x] Wolves brain: affordance availability check ([F6](forks.md#f6)), then on
      Thirsty/Dehydrated MOVE_TO the nearest water and issue `EXECUTE_INTERACTION` on arrival,
      latched once per band crossing. Acceptance: drill-scaled ([I10](issues.md#i10)) — walk,
      drink, +3, Thirsty clears, one log arc. → the latch matured into SIP PACING (re-arm when
      the row's `set_tic` advances): `NPC_THIRST=12` → Thirsty → walk (106,60)→(102,68) → sips
      +3.0 to 35.33 → `conditions=[]` → `["quenched"]` mood 0.7 — one unprompted log arc; the
      wolf even re-sipped when depletion dipped it back under 35 (a working thermostat).
- [x] The panel shows the arc with ZERO client changes beyond the shared eval: Thirsty card →
      walk → Quenched card. Acceptance: browser captures of both card states during the drill.
      → captured live at `:5174/?focus=102,68`: wolf `0x30800000` mood 35% with the
      **Thirsty −0.15** card; wolf `0x30800001` (the sipper) mood 70% with **Quenched +0.20
      3416t** (remaining tics rendering). One client line changed (`conditionLabelOf(ref)`).

## P5 — the verdict

- [x] Docs + memory truth pass: component docs re-pointed where they touch needs/actions, the
      needs-moodlets memory amended (i8 domain, registry ids), the stream memory written.
      Acceptance: `bin/rd docs-check` green. → component docs were clean (grep found no stale
      needs spellings — P0 already moved the authoritative sections); memories:
      needs-moodlets-delivered rewritten to the f32/refs reality, interactions-delivered
      written, toml-content's stale explicit-id claim superseded, index rows updated; the
      work index row records delivery. docs-check green.
- [ ] Cold-boot the stack; standing drills (wolf trip, thirst crossing, panel cards) + the
      UNPROMPTED drink arc green together; **the user's eyes close the stream**. Acceptance:
      captures + logs recorded in `completed.md`.
