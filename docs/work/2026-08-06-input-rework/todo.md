# Plan — input-rework

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the contract, documented before built

- [ ] VARIABLES.md § TOML schema: `menu_text` (default = label), location `"target"`
      ([F4](forks.md#f4)), the `move` effect ([F6](forks.md#f6)), the RESERVED input
      vocabulary (`pawn`/`destination`/`amount`, [F5](forks.md#f5)), drink's re-authored
      two-input signature, `speed`'s removal. Acceptance: every P1 field has a spelling first.
- [ ] ACTIONS.md: `MOVE_TO` marked WORKER-ONLY (leaves `CLIENT_VERBS`; the interaction arm its
      only composer — [F3](forks.md#f3)); §Movement's front door re-written around
      `EXECUTE_INTERACTION(move_to)`; the seed's serial/speculation notes updated
      ([I1](issues.md#i1)/[I2](issues.md#i2)). Acceptance: `bin/rd docs-check` green.
- [ ] The INPUT contract into `docs/components/client/webgl/design/` (right = active
      selection, left = context pie menu, dismissal rules, one menu, DOM overlay —
      [F1](forks.md#f1)/[F7](forks.md#f7)). Acceptance: docs-check green; the user's spelling
      quoted.

## P1 — loader + corpus

- [ ] Loader: `menu_text`, the location set `{"on","target"}`, `MoveEffect` with `@ref`
      resolution, the no-effect refusal generalized (an interaction needs `satisfy` OR `move`).
      Acceptance: crate tests per addition incl. refusals.
- [ ] Corpus: `[[interaction]] move_to` (menu_text "Move To", `affordances =
      ["can_move_ground"]`, `inputs = ["pawn","destination"]`, the move effect, location
      "target"); grass/dirt/sand/stone/water carry it, wall_smooth does NOT
      ([F9](forks.md#f9)); drink gains `menu_text = "Drink"` + the baked-need two-input
      signature ([F5](forks.md#f5)). Acceptance: loads; `rd content-check` green.
- [ ] Extend the golden dump (menu_text, move effects, locations, the new carrier lists,
      drink's signature) and re-bless; 2-pass gate green. Acceptance: every diff line
      accounted for in completed.md ([I8](issues.md#i8)).

## P2 — the server rewire

- [ ] Worker: the `move` arm — resolve the effect, validate location `"target"` (the
      DESTINATION tile offers the interaction, per-biome merged read — [I6](issues.md#i6)),
      gate on predicates, queue `PROMOTE MOVE_TO pawn dest` at `master+4`. Acceptance: a
      hand-fired event walks the wolf; two rapid fires leave ONE chain ([I2](issues.md#i2)).
- [ ] Worker: chain spacing from the DERIVED `ground_speed` (the pawn's fanned rows + corpus
      through `stat_eval`); the speeds table + `tics_for` deleted ([F8](forks.md#f8)).
      Acceptance: hop spacing 12 tics in the logs; `grep` finds no speeds table.
- [ ] npc: wander + drink trips compose `EXECUTE_INTERACTION(move_to)`; deadlines from the
      derived stat; the legacy `wildlife` brain DELETED ([F10](forks.md#f10)). Acceptance:
      the soak runs trips + the unprompted drink arc through the new door only.
- [ ] Edge: `MOVE_TO` leaves `CLIENT_VERBS` ([I3](issues.md#i3) — LAST, after both callers).
      Acceptance: a hand-queued raw MOVE_TO is rejected at the door; trips still flow.

## P3 — the client rewire

- [ ] WorldScene: RIGHT click = `selectAt` (panel + outlines — the whole current left-click
      behavior); the left-click select path removed; middle-pan + build-mode branches
      unchanged. Acceptance: browser — right-click selects pawn/thing/tile, the panel follows.
- [ ] Speculation speed from the derived stat: a wasm accessor over `(payload, needs, now)`
      for `ground_speed`; `MoverLayer.speedFor` moves onto it; `speed` field +
      `thing_speed(s)`/`thingSpeed` + stat-model's I10 guard DELETED ([F8](forks.md#f8)/
      [I7](issues.md#i7)). Acceptance: the glide still runs 12 tics/tile; delete-greps clean.
- [ ] The client move path composes `EXECUTE_INTERACTION(move_to)` (corpus-resolved ref);
      `move_to_program`/`Command::Move`/`moveEntity` deleted through core/wasm/TS.
      Acceptance: grep `move_to_program` empty; a scripted move from the console glides.

## P4 — the pie menu

- [ ] Availability: for a left-clicked tile/thing, the carrier's interactions filtered by the
      ACTIVE pawn's predicates (`interaction_available` via wasm over its fanned rows) AND the
      location rule ([F4](forks.md#f4)/[I4](issues.md#i4)). Acceptance: wolf selected — water
      under it yields Drink + Move To; water elsewhere yields Move To only; no selection →
      nothing.
- [ ] The overlay: `menu_text` rounded rects at a fixed radius, equal angles from 12 o'clock,
      anchored at the click, ConditionCards DOM pattern, ONE menu ([F7](forks.md#f7)).
      Acceptance: browser captures at 1 and 2 options.
- [ ] Dismissal + execution ([I5](issues.md#i5)): a rect click composes (reserved vocabulary,
      [F5](forks.md#f5)) + queues + hides; ANY other input hides; menu clicks never reach the
      canvas. Acceptance: browser — Move To walks the wolf, Drink sips it, a stray click just
      closes the menu.
- [ ] The npc soak + panel run unchanged beside menu use ([I10](issues.md#i10) accepted: the
      wolf wanders off after a player order — supersession, not a bug). Acceptance: standing
      drills green in the same session as menu-driven orders.

## P5 — the verdict

- [ ] Docs + memory truth pass; delete-greps clean (`MOVE_TO` in CLIENT_VERBS,
      `move_to_program`, `moveEntity`, `thingSpeed`, `speed =` on things, wildlife).
      Acceptance: `bin/rd docs-check` green; greps recorded in completed.md.
- [ ] Cold-boot the stack; by-hand browser drills (right-click select, pie menu Move To +
      Drink) + the npc's unprompted arc green together; **the user's eyes close the stream**.
      Acceptance: captures + logs in completed.md.
