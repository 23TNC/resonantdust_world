# Plan — input-rework

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the contract, documented before built

- [x] VARIABLES.md § TOML schema: `menu_text` (default = label), location `"target"`
      ([F4](forks.md#f4)), the `move` effect ([F6](forks.md#f6)), the RESERVED input
      vocabulary (`pawn`/`destination`/`amount`, [F5](forks.md#f5)), drink's re-authored
      two-input signature, `speed`'s removal. Acceptance: every P1 field has a spelling first.
      → the schema block gained a full `[[interaction]] move_to` sample beside the re-authored
      drink; the wolf sample's `speed` line replaced by the derived-ground_speed note; the
      water carrier sample shows `{ name = "move_to" }` + the walls-don't-carry-it note.
- [x] ACTIONS.md: `MOVE_TO` marked WORKER-ONLY (leaves `CLIENT_VERBS`; the interaction arm its
      only composer — [F3](forks.md#f3)); §Movement's front door re-written around
      `EXECUTE_INTERACTION(move_to)`; the seed's serial/speculation notes updated
      ([I1](issues.md#i1)/[I2](issues.md#i2)). Acceptance: `bin/rd docs-check` green. → the
      verb row + §Movement rewritten (front door, derived-stat spacing, seed serial = the
      worker-queued event's reference, MoveIntent's event_tic = master+4); the stale
      "continuation `MOVE_TO`" wording fixed to `MOVE_STEP` in passing. docs-check green.
- [x] The INPUT contract into `docs/components/client/webgl/design/` (right = active
      selection, left = context pie menu, dismissal rules, one menu, DOM overlay —
      [F1](forks.md#f1)/[F7](forks.md#f7)). Acceptance: docs-check green; the user's spelling
      quoted. → `design/input-model.md`: the button table, menu content/composition/
      presentation/lifecycle, the empty-set rule, and the recorded non-goals.

## P1 — loader + corpus

- [x] Loader: `menu_text`, the location set `{"on","target"}`, `MoveEffect` with `@ref`
      resolution, the no-effect refusal generalized (an interaction needs `satisfy` OR `move`).
      Acceptance: crate tests per addition incl. refusals. → 39/39 lib tests;
      `the_input_rework_fields_round_trip` covers menu_text default, "target", the resolved
      move effect, the zero-magnitude carrier bind, and the dangling-@ref + no-effect refusals.
- [x] Corpus: `[[interaction]] move_to` (menu_text "Move To", `affordances =
      ["can_move_ground"]`, `inputs = ["pawn","destination"]`, the move effect, location
      "target"); grass/dirt/sand/stone/water carry it, wall_smooth does NOT
      ([F9](forks.md#f9)); drink gains `menu_text = "Drink"` + the baked-need two-input
      signature ([F5](forks.md#f5)). Acceptance: loads; `rd content-check` green. → authored
      exactly; content-check clean (7 files).
- [x] Extend the golden dump (menu_text, move effects, locations, the new carrier lists,
      drink's signature) and re-bless; 2-pass gate green. Acceptance: every diff line
      accounted for in completed.md ([I8](issues.md#i8)). → 9+/6−, all reviewed: move_to's
      params + seed ref `0x80040020`, drink's `Name("thirst")` two-input satisfy, menu_text
      on both, the five carrier lists, `wall_smooth []` byte-checked. Native 39+3, wasm32
      pkg rebuilt clean.

## P2 — the server rewire

- [x] Worker: the `move` arm — resolve the effect, validate location `"target"` (the
      DESTINATION tile offers the interaction, per-biome merged read — [I6](issues.md#i6)),
      gate on predicates, queue `PROMOTE MOVE_TO pawn dest` at `master+4`. Acceptance: a
      hand-fired event walks the wolf; two rapid fires leave ONE chain ([I2](issues.md#i2)).
      → the arm generalized (target from either effect, carrier tile BY the location rule,
      satisfy scoped, the seed `[PROMOTE_EVENT, PROMOTE, MOVE_TO, pawn, dest]` appended);
      LIVE: `interaction executed … moved=true dest=(97,64)` and the wolf WALKED (npc trips
      below). The two-rapid-fires supersession re-drill rides P4's browser items (I2 — no
      natural supersession occurred this soak; the machinery is untouched).
- [x] Worker: chain spacing from the DERIVED `ground_speed` (the pawn's fanned rows + corpus
      through `stat_eval`); the speeds table + `tics_for` deleted ([F8](forks.md#f8)).
      Acceptance: hop spacing 12 tics in the logs; `grep` finds no speeds table. → the
      `ground_speed_tics` closure (per-hop derivation — a mid-trip stat change slows the
      chain); `load_corpus` returns just the Bundle; LIVE: a 7-hop trip took 16 s = 8
      hop-slots × 12 tics at 6 Hz, exactly the derived pace; `speeds`/`tics_for` gone.
- [x] npc: wander + drink trips compose `EXECUTE_INTERACTION(move_to)`; deadlines from the
      derived stat; the legacy `wildlife` brain DELETED ([F10](forks.md#f10)). Acceptance:
      the soak runs trips + the unprompted drink arc through the new door only. →
      `issue_move` composes the F5 two-input event; `derived_speed()` from payload rows +
      active set; `resolve_thing_in` returns the def alone (the speed half gone; the pin
      test now derives 12.0 from the walks binding); wildlife.rs deleted with its dispatch
      arm. LIVE: wander trips + arrivals flow, ONE MoveIntent per trip (I1 ✓).
- [x] Edge: `MOVE_TO` leaves `CLIENT_VERBS` ([I3](issues.md#i3) — LAST, after both callers).
      Acceptance: a hand-queued raw MOVE_TO is rejected at the door; trips still flow. →
      EXECUTED AFTER P3's client swap per I3 (the plan's in-P2 position was the I3 hazard —
      re-ordered, not skipped). FOUND LIVE en route: the first drill's raw MOVE_TO WALKED
      the wolf — the redeploy's edge build hit the docker/WSL2 MTIME MISS ([[docker-cargo-
      mtime-miss]] verbatim; `touch` + rebuild showed `Compiling edge`). On the really-new
      binary: raw MOVE_TO → no chain, wolf stationary 6 s+; menu + npc trips flow.

## P3 — the client rewire

- [x] WorldScene: RIGHT click = `selectAt` (panel + outlines — the whole current left-click
      behavior); the left-click select path removed; middle-pan + build-mode branches
      unchanged. Acceptance: browser — right-click selects pawn/thing/tile, the panel follows.
      → verified live: button-2 pointerdown selects a tile (panel `tile 101, 62`) and the
      wolf (silhouette outline + full pawn panel). NOTE: the MCP browser's `right_click`
      doesn't deliver pointerdown — drilled via synthetic `PointerEvent{button: 2}` through
      the same handler a human's right-click reaches.
- [x] Speculation speed from the derived stat: a wasm accessor over `(payload, needs, now)`
      for `ground_speed`; `MoverLayer.speedFor` moves onto it; `speed` field +
      `thing_speed(s)`/`thingSpeed` + stat-model's I10 guard DELETED ([F8](forks.md#f8)/
      [I7](issues.md#i7)). Acceptance: the glide still runs 12 tics/tile; delete-greps clean.
      → `pawnGroundSpeed(payload, needs, now)`; `speedFor` keyed by ENTITY; the panel shows
      `speed 12 tics/tile` DERIVED and menu-ordered trips glide; humans re-bound to walks
      level 1 (24 t/t — the old 16 has no walks slot; noted). Golden: speeds section gone.
- [x] The client move path composes `EXECUTE_INTERACTION(move_to)` (corpus-resolved ref);
      `move_to_program`/`Command::Move`/`moveEntity` deleted through core/wasm/TS.
      Acceptance: grep `move_to_program` empty; a scripted move from the console glides. →
      `composeInteraction` (wasm) + the F5 vocabulary composer in `WorldScene.fireOption`;
      `Command::Move`/`move_to_program`/`moveEntity`/`moveSelf` deleted through
      core/wasm/WasmClient/WorldBridge; menu-driven moves glide (spec armed to the dest).

## P4 — the pie menu

- [x] Availability: for a left-clicked tile/thing, the carrier's interactions filtered by the
      ACTIVE pawn's predicates (`interaction_available` via wasm over its fanned rows) AND the
      location rule ([F4](forks.md#f4)/[I4](issues.md#i4)). Acceptance: wolf selected — water
      under it yields Drink + Move To; water elsewhere yields Move To only; no selection →
      nothing. → `tileMenuOptions`/`thingMenuOptions` (ONE wasm filter; the carrier lookup is
      the bridge's autotile `tileKindAt` map, exposed as `tileDefAt`); LIVE: water
      not-standing = ["Move To"], standing = ["Drink", "Move To"], no selection/thing/pawn
      target/unstreamed ground = nothing.
- [x] The overlay: `menu_text` rounded rects at a fixed radius, equal angles from 12 o'clock,
      anchored at the click, ConditionCards DOM pattern, ONE menu ([F7](forks.md#f7)).
      Acceptance: browser captures at 1 and 2 options. → `PieMenu.ts`; captures — one option
      ("Move To" at 12 o'clock) and two (Drink at 12, Move To at 6, radius 72). Found+fixed
      live: the rects rendered UNDER the canvas (panel z-bands reach 50k — the menu sits at
      60k) and drifted off-cursor (#app is a transformed containing block — hosted on BODY).
- [x] Dismissal + execution ([I5](issues.md#i5)): a rect click composes (reserved vocabulary,
      [F5](forks.md#f5)) + queues + hides; ANY other input hides; menu clicks never reach the
      canvas. Acceptance: browser — Move To walks the wolf, Drink sips it, a stray click just
      closes the menu. → REAL mouse clicks: Move To → `move_to … dest=(102,68)` executed and
      the wolf walked+glided there; Drink → `satisfied=Some("thirst") from=18.48 to=21.48
      grants=1`; a canvas click closed the menu (document-capture listeners); TWO rapid
      orders → ONE chain survived (rest at the second dest exactly — I2 supersession under
      the worker-queued seed serials).
- [x] The npc soak + panel run unchanged beside menu use ([I10](issues.md#i10) accepted: the
      wolf wanders off after a player order — supersession, not a bug). Acceptance: standing
      drills green in the same session as menu-driven orders. → npc trips (wolf …001) and a
      menu order (wolf …000 → (105,64)) executing side by side in the worker log; the drink
      thermostat + panel cards ran all session. FOUND+FIXED live ([I11](issues.md#i11)): the
      universal move_to carrier broke the npc's drink filter — it fired move_to with the
      3-input drink shape; the drink pass now filters to SATISFY carriers and
      `fire_interaction` binds by the F5 vocabulary.

## P5 — the verdict

- [ ] Docs + memory truth pass; delete-greps clean (`MOVE_TO` in CLIENT_VERBS,
      `move_to_program`, `moveEntity`, `thingSpeed`, `speed =` on things, wildlife).
      Acceptance: `bin/rd docs-check` green; greps recorded in completed.md.
- [ ] Cold-boot the stack; by-hand browser drills (right-click select, pie menu Move To +
      Drink) + the npc's unprompted arc green together; **the user's eyes close the stream**.
      Acceptance: captures + logs in completed.md.
