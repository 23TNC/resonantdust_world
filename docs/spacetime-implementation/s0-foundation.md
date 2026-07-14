# S0 — Foundation: decisions + codec

**Goal:** lock the blocking decisions and land the pure event-word codec. No behavior change;
everything additive (like `object.rs` landed beside `refs.rs`).

**Status:** ✅ **done** — `shared/codec/src/event_word.rs` (word frame + `op_code`s + action-id
palette) landed with roundtrip / all-ones-disjoint / low-48-mask tests; `cargo test -p
resonantdust-codec` green (33 tests). **Depends on:** nothing.

## Decisions — all ✅ confirmed

- **D1 · Branch (`?:`) encoding.** ✅ A forward-only `SKIP` action word carrying a `LITERAL`
  word-count: `AWAIT` pushes a boolean, `?` skips the *else* run when true / the *then* run
  when false. **No backward jumps** → execution is bounded by construction (termination for
  fenced replay).
- **D2 · Word tag = `op_code`.** ✅ The tag is named **`op_code`** — *not* `kind`, to avoid
  colliding with the object model's `kind_reference`/`kind_id`. `op_code:4` =
  `LITERAL 0 | OBJECT 1 | ACTION 2 | ALIAS 3`; control (`SKIP`, `AWAIT`, `FAIL`) are
  `action_reference`s carried by `ACTION` words, not new op_codes; 4–15 reserved. The verb
  decides cold-vs-hot; no separate operand op_code. (Each word *is* a stack instruction, so its
  tag is that instruction's opcode — push-literal / push-object / do-action / push-alias.)
- **D3 · Hot identity = a real reference.** ✅ `entity_key` isn't an opaque key — today it *is*
  the `u64` **`entity_reference`** (minted or positional) from
  [`refs.rs`](../../shared/codec/src/refs.rs), which is exactly "something we actually have."
  Keep it as the internal `state`/`state_log` PK for now; expose `hot_reference:u32` as the
  operand form (the `entity_id` slice + `server_reference`). Any re-key of `state` to the `u32`
  `hot_reference` is **[S7](s7-tail.md)**, off the critical path — but the identity is a named
  reference at every step, never a placeholder.
- **D4 · `server_reference` layout.** ✅ Keep functional `server_type:6 | server_id:10`
  (`refs.rs`) — the worker's routing reads `server_type`. The DSL word's 16-bit
  `server_reference` works under either layout, so the geographic redesign is not a blocker.
- **D5 · Purpose-built VM (no `shared/dsl` reuse).** ✅ [`shared/dsl`](../../shared/dsl/src) is
  a **text-parsed (`.rd`) VM for tile content/visuals** — it builds a `Cell` tree of render
  prims. Different syntax (source text, not `u64` words), value model (visual `Cell`s, not game
  references/state), and purpose. The only overlap is the abstract "postfix stack + dispatch
  loop," which is a few lines, not a library. Build the event VM **purpose-built** in the
  worker; reuse `shared/dsl` only if a genuine shared primitive later falls out.

## Changes

Add to [`shared/codec`](../../shared/codec/src) (e.g. `event_word.rs`), pure integer math:

- `pack_word(op_code, server_reference, payload) -> u64` + accessors `word_op_code`,
  `word_server_reference`, `word_payload` — the frame
  `op_code:4 | reserved:12 | server_reference:16 | payload:32`, with the low-48 plain-mask
  extraction guarantee (`payload = w & 0xFFFF_FFFF`, `server = (w>>32) & 0xFFFF`).
- The `op_code` constants (D2) and the **action_reference** op-id palette (`MOVE`, `INSPECT`,
  `SPAWN`, `PACK`, `AWAIT`, `SKIP`, `FAIL`, …) — append-only, keyed into the existing
  `action_reference` shape (`data_type:6 | action_id:10`) extended across the `u32` payload.
  (No `MINT`/`GET` verbs — cold→hot of a target is absorbed into enqueue, [S3](s3-worker.md).)

## Verify

`cargo test -p resonantdust-codec` (in-container via `rd`). Roundtrip + all-ones-disjoint tests
in the `object.rs` style. Nothing else moves; stack stays green.

## Refs

Word frame + kinds: [event-dsl.md](../spacetime-tables/event-dsl.md). Reference layouts:
[../references/](../references/).
