# Plan — conditions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the one open call in [`blockers.md`](blockers.md)._

## P0 — the corpus and the shared crate speak "condition"

- [x] Rename `[[moodlet]]` → `[[condition]]` and each band's `moodlet = ` key → `condition = ` in
      `content/needs.toml`, ids untouched. Acceptance: `grep -in moodlet content/` is empty; ids
      1/2/3 still map to thirsty/dehydrated/quenched.
- [x] Rename `MoodletParams` → `ConditionParams`, `NeedBand.moodlet` → `.condition`, and every
      `moodlet_*` accessor on `Bundle` to `condition_*` in `shared/content/src/loader.rs`.
      Acceptance: `cargo check -p resonantdust-content` green; no `moodlet` identifier remains in
      the file.
- [x] Rewrite the loader's "conditional vs stored" doc comments as **derived vs timed**
      ([F4](forks.md#f4)). Acceptance: `grep -in conditional shared/content/src` returns nothing
      outside the biome-rule code; `duration == 0` is documented as DERIVED.
- [x] Rename the TOML structs + registry wiring (`MoodletToml`, `all.moodlet`, `moodlet_slots`)
      in `shared/content/src/toml_loader.rs` to the `condition` spelling. Acceptance:
      `cargo test -p resonantdust-content` green including its inline corpus test.
- [x] Rename `active_moodlets` → `active_conditions` and its `ActiveMoodlet` / `moodlet_id` types
      in `shared/content/src/needs_eval.rs`. Acceptance: `cargo test -p resonantdust-content`
      green; band + timed-grant unit tests pass unchanged in value.
- [x] Re-bless the golden fixture under the new names (`BLESS_GOLDEN=1 cargo test -p
      resonantdust-content --test golden`). Acceptance: the fixture diff is name-only — every
      number and id in `tests/golden/corpus.txt` is unchanged; the un-blessed test then passes.

## P1 — the wire names follow (values do not move)

- [x] Rename `PAYLOAD_OP_MOODLET` → `PAYLOAD_OP_CONDITION` and `moodlet_word` /
      `payload_moodlets` / `upsert_moodlet` to the `condition` spelling in
      `shared/codec/src/payload.rs`, value still `3`. Acceptance: `cargo test -p
      resonantdust-codec` green; `PAYLOAD_OP_CONDITION == 3`.
- [x] Rename `GRANT_MOODLET` → `GRANT_CONDITION` in `shared/codec/src/action.rs`, opcode still
      `11` and arity still 2. Acceptance: `cargo test -p resonantdust-codec` green; the arity
      table still yields `&[Write, Imm]` for `11`.
- [x] Rename the pawn module's `grant_moodlet` reducer → `grant_condition` and its `MOODLET`
      payload splice in `server/spacetime/server/modules/pawn/src/lib.rs`. Acceptance: `bin/rd
      deploy module pawn` publishes without a schema error.
- [x] Regenerate `server/st-bindings/src/pawn/` and `server/edge/src/bindings/pawn/` against the
      republished module. Acceptance: `grant_condition_reducer.rs` exists,
      `grant_moodlet_reducer.rs` is deleted, `cargo check -p resonantdust-edge` green.
- [x] Rename the `GRANT_MOODLET` verb in the edge allowlist + relay (`server/edge/src/ws.rs`) and
      the worker relay (`server/worker/src/main.rs`). Acceptance: `bin/rd redeploy --run` clean; a
      client-issued grant still lands (edge log shows the verb accepted).
- [x] Rename the `moodlet` identifiers in `client/npc/src/lib.rs` and `brains/wolves.rs`.
      Acceptance: `cargo check -p resonantdust-npc` green; `bin/sim run npc` logs a band flip
      naming a "condition".

## P2 — the client and the authoritative docs

- [x] Rename `pawn_moodlets` / `moodlet_labels` → `pawn_conditions` / `condition_labels` and their
      js_names in `shared/wasm/src/lib.rs`. Acceptance: `bin/rd build shared` green (native +
      wasm32); the generated `.d.ts` exports `pawnConditions` and `conditionLabels`.
- [x] Rename `moodlets` → `conditions` through `WasmClient.ts`, `MoverLayer.ts`, `WorldScene.ts`
      and `DetailsPanel.ts`'s `DetailsProviders`. Acceptance: `grep -rin moodlet client/webgl/src`
      is empty; the client builds and a selected wolf still lists its conditions as text rows.
- [x] Rename the moodlet vocabulary in `docs/VARIABLES.md` §"Needs & moodlets", `docs/TABLES.md`
      (the `MOODLET` payload row + verb prose) and `docs/ACTIONS.md` row 11, adopting derived/timed
      ([F4](forks.md#f4)). Acceptance: `bin/rd docs-check` green; the three files name
      `CONDITION` / `GRANT_CONDITION` with unchanged widths, opcodes and arities.
- [x] State in `docs/VARIABLES.md` that a condition's effects are an OPEN set, `mood` being the
      first ([F6](forks.md#f6)). Acceptance: the block names mood as one effect field, not as the
      definition of a condition, and points at the successor stream.

## P3 — priority becomes authored data

- [x] Add an optional `priority` (integer, default `0`) to `ConditionParams` + `ConditionToml` and
      author it on thirsty/dehydrated/quenched ([F2](forks.md#f2)). Acceptance: a loader unit test
      reads the authored value; an omitted key yields `0`.
- [x] Make `active_conditions` return its result sorted by `priority` desc, then `|mood|` desc,
      then `condition_id` asc ([F3](forks.md#f3)). Acceptance: a unit test with a deliberately
      shuffled input asserts the exact emitted order.
- [x] Widen the wasm `pawnConditions` stride from 3 to 4 (`id, mood, remaining, priority`) and
      re-thread the TS decode. Acceptance: the panel's conditions arrive already ordered — the TS
      does no sorting of its own.
- [x] Document `priority` in `docs/VARIABLES.md`'s conditions block. Acceptance: `bin/rd
      docs-check` green; the block states the three-key sort and that priority is corpus-tunable.

## P4 — the strip is a sibling that draws past the panel ([F7](forks.md#f7))

- [x] Drop the condition rows out of `DetailsPanel.render`'s text and give the remaining rows their
      own `<pre>` child. Acceptance: the panel body shows the text rows only; `grep` finds no
      condition row emitted into the body.
- [x] Create the strip as a `position: fixed` element appended to the panel's HOST — a SIBLING of
      the panel root, z-index above the panel band. Acceptance: the DOM inspector shows it as a
      sibling, and a test-wide strip paints over the world past the panel's right edge.
- [x] Anchor the strip to the panel's bottom-left with the authored bottom + left padding,
      re-anchoring off the panel's `rectChange`. Acceptance: drag, corner-resize, snap-flip and a
      window resize each keep the strip glued to the panel's bottom-left corner.
- [x] Mirror the panel's lifecycle: hide on minimize / hide / close / empty selection, and remove
      the element in `destroy()`. Acceptance: minimizing the panel hides the strip; closing it
      leaves no orphan node in the DOM.
- [x] Build a card element factory (label, signed mood, remaining-tics timer) styled from the panel
      chrome constants. Acceptance: one card renders with all three fields legible at the panel's
      12px monospace.
- [x] Lay the cards out as a horizontal flex row with the authored gap between cards, sized so 4
      maximized cards fit the panel's default width. Acceptance: measured in the browser, the gaps
      match the constants and 4 cards span the default panel width minus its padding.
- [x] Render the top 4 by priority maximized and the remainder minimized at the authored width
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

## P6 — the strip behaves at the screen edge ([B1](blockers.md#b1) resolved)

- [ ] Clamp the strip at the VIEWPORT edge — past it, scroll inside the strip
      ([B1](blockers.md#b1) method #1 as the inner fallback). Acceptance: 12 conditions expanded in
      an 800px window — every card reachable, and `document.body` never scrolls horizontally.
- [ ] Make the strip's empty area pointer-transparent so it does not steal world input. Acceptance:
      a right-click on world tiles under the strip's gaps still issues a move; a click on a card
      does not.
- [ ] Verify the strip against a panel snapped to the RIGHT edge of the screen. Acceptance: the
      cards extend past the panel and stay on-screen (clamped), not off the right of the viewport.
- [ ] Run the wolf drill end to end on the TOML corpus with the cards live. Acceptance: a wolf runs
      full → Dehydrated, cards appear/disappear at the computed crossing tics, and the user's eyes
      close the stream.
