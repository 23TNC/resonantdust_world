# Plan — conditions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the one open call in [`blockers.md`](blockers.md)._

## P0 — the corpus and the shared crate speak "condition"

- [ ] Rename `[[moodlet]]` → `[[condition]]` and each band's `moodlet = ` key → `condition = ` in
      `content/needs.toml`, ids untouched. Acceptance: `grep -in moodlet content/` is empty; ids
      1/2/3 still map to thirsty/dehydrated/quenched.
- [ ] Rename `MoodletParams` → `ConditionParams`, `NeedBand.moodlet` → `.condition`, and every
      `moodlet_*` accessor on `Bundle` to `condition_*` in `shared/content/src/loader.rs`.
      Acceptance: `cargo check -p resonantdust-content` green; no `moodlet` identifier remains in
      the file.
- [ ] Rewrite the loader's "conditional vs stored" doc comments as **derived vs timed**
      ([F4](forks.md#f4)). Acceptance: `grep -in conditional shared/content/src` returns nothing
      outside the biome-rule code; `duration == 0` is documented as DERIVED.
- [ ] Rename the TOML structs + registry wiring (`MoodletToml`, `all.moodlet`, `moodlet_slots`)
      in `shared/content/src/toml_loader.rs` to the `condition` spelling. Acceptance:
      `cargo test -p resonantdust-content` green including its inline corpus test.
- [ ] Rename `active_moodlets` → `active_conditions` and its `ActiveMoodlet` / `moodlet_id` types
      in `shared/content/src/needs_eval.rs`. Acceptance: `cargo test -p resonantdust-content`
      green; band + timed-grant unit tests pass unchanged in value.
- [ ] Re-bless the golden fixture under the new names (`BLESS_GOLDEN=1 cargo test -p
      resonantdust-content --test golden`). Acceptance: the fixture diff is name-only — every
      number and id in `tests/golden/corpus.txt` is unchanged; the un-blessed test then passes.

## P1 — the wire names follow (values do not move)

- [ ] Rename `PAYLOAD_OP_MOODLET` → `PAYLOAD_OP_CONDITION` and `moodlet_word` /
      `payload_moodlets` / `upsert_moodlet` to the `condition` spelling in
      `shared/codec/src/payload.rs`, value still `3`. Acceptance: `cargo test -p
      resonantdust-codec` green; `PAYLOAD_OP_CONDITION == 3`.
- [ ] Rename `GRANT_MOODLET` → `GRANT_CONDITION` in `shared/codec/src/action.rs`, opcode still
      `11` and arity still 2. Acceptance: `cargo test -p resonantdust-codec` green; the arity
      table still yields `&[Write, Imm]` for `11`.
- [ ] Rename the pawn module's `grant_moodlet` reducer → `grant_condition` and its `MOODLET`
      payload splice in `server/spacetime/server/modules/pawn/src/lib.rs`. Acceptance: `bin/rd
      deploy module pawn` publishes without a schema error.
- [ ] Regenerate `server/st-bindings/src/pawn/` and `server/edge/src/bindings/pawn/` against the
      republished module. Acceptance: `grant_condition_reducer.rs` exists,
      `grant_moodlet_reducer.rs` is deleted, `cargo check -p resonantdust-edge` green.
- [ ] Rename the `GRANT_MOODLET` verb in the edge allowlist + relay (`server/edge/src/ws.rs`) and
      the worker relay (`server/worker/src/main.rs`). Acceptance: `bin/rd redeploy --run` clean; a
      client-issued grant still lands (edge log shows the verb accepted).
- [ ] Rename the `moodlet` identifiers in `client/npc/src/lib.rs` and `brains/wolves.rs`.
      Acceptance: `cargo check -p resonantdust-npc` green; `bin/sim run npc` logs a band flip
      naming a "condition".

## P2 — the client and the authoritative docs

- [ ] Rename `pawn_moodlets` / `moodlet_labels` → `pawn_conditions` / `condition_labels` and their
      js_names in `shared/wasm/src/lib.rs`. Acceptance: `bin/rd build shared` green (native +
      wasm32); the generated `.d.ts` exports `pawnConditions` and `conditionLabels`.
- [ ] Rename `moodlets` → `conditions` through `WasmClient.ts`, `MoverLayer.ts`, `WorldScene.ts`
      and `DetailsPanel.ts`'s `DetailsProviders`. Acceptance: `grep -rin moodlet client/webgl/src`
      is empty; the client builds and a selected wolf still lists its conditions as text rows.
- [ ] Rename the moodlet vocabulary in `docs/VARIABLES.md` §"Needs & moodlets", `docs/TABLES.md`
      (the `MOODLET` payload row + verb prose) and `docs/ACTIONS.md` row 11, adopting derived/timed
      ([F4](forks.md#f4)). Acceptance: `bin/rd docs-check` green; the three files name
      `CONDITION` / `GRANT_CONDITION` with unchanged widths, opcodes and arities.
- [ ] State in `docs/VARIABLES.md` that a condition's effects are an OPEN set, `mood` being the
      first ([F6](forks.md#f6)). Acceptance: the block names mood as one effect field, not as the
      definition of a condition, and points at the successor stream.

## P3 — priority becomes authored data

- [ ] Add an optional `priority` (integer, default `0`) to `ConditionParams` + `ConditionToml` and
      author it on thirsty/dehydrated/quenched ([F2](forks.md#f2)). Acceptance: a loader unit test
      reads the authored value; an omitted key yields `0`.
- [ ] Make `active_conditions` return its result sorted by `priority` desc, then `|mood|` desc,
      then `condition_id` asc ([F3](forks.md#f3)). Acceptance: a unit test with a deliberately
      shuffled input asserts the exact emitted order.
- [ ] Widen the wasm `pawnConditions` stride from 3 to 4 (`id, mood, remaining, priority`) and
      re-thread the TS decode. Acceptance: the panel's conditions arrive already ordered — the TS
      does no sorting of its own.
- [ ] Document `priority` in `docs/VARIABLES.md`'s conditions block. Acceptance: `bin/rd
      docs-check` green; the block states the three-key sort and that priority is corpus-tunable.

## P4 — conditions render as cards

- [ ] Split `DetailsPanel.render` so the text rows live in one `<pre>` child and the conditions get
      their own container element. Acceptance: the panel looks identical to today (text conditions
      still listed), with the conditions emitted from the new container.
- [ ] Build a `ConditionCard` element factory (label, signed mood, remaining-tics timer) styled
      from the panel chrome constants. Acceptance: one card renders with all three fields legible
      at the panel's 12px monospace.
- [ ] Lay the cards out as a horizontal flex strip pinned to the bottom of the body, with padding
      from the bottom edge, from the left edge, and between cards. Acceptance: measured in the
      browser, the gaps match the authored constants and the first card's left gap equals the
      body's left padding.
- [ ] Raise the details panel's `minWidth` so 4 maximized cards plus all padding fit at minimum
      size. Acceptance: dragging the panel to its smallest width still shows 4 whole cards, no
      clipping.
- [ ] Render the top 4 by priority maximized and the remainder minimized at the authored width
      fraction. Acceptance: with 6 conditions forced onto a wolf, cards 1–4 are full width in
      priority order and 5–6 are the narrow form.

## P5 — click to maximize all

- [ ] Add a panel-scoped `conditionsExpanded` flag, default off, persisted with the panel's other
      prefs. Acceptance: toggling it and reloading the client restores the chosen state.
- [ ] Wire a click on any minimized card to set the flag (all cards maximize) and a click on any
      card while expanded to clear it ([F5](forks.md#f5)). Acceptance: click a narrow card → every
      card is full width; click again → back to 4-maximized.
- [ ] Give the cards a cursor + hover affordance so the click target reads as clickable.
      Acceptance: hovering a card changes the cursor and the card's chrome.

## P6 — the overflow method — BLOCKED on [B1](blockers.md#b1)

- [ ] Implement the user's chosen overflow method for the expanded strip. Acceptance: with 8
      conditions expanded, every card is reachable and the chosen behaviour matches what
      [B1](blockers.md#b1) records as decided.
- [ ] Run the wolf drill end to end on the TOML corpus with the cards live. Acceptance: a wolf runs
      full → Dehydrated, cards appear/disappear at the computed crossing tics, and the user's eyes
      close the stream.
