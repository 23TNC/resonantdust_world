# Intent — `event_reference` is a `u32`

_Why the shape is what it is. Last updated: 2026-07-14._

**Shape (design):** `event_reference` is a **`u32`**. (See [`design/tables.md`](../design/tables.md)
— the `event_log` key — and [`design/event-dsl.md`](../design/event-dsl.md) — the `ALIAS` word
carries it.)

**Why:** a `u32` `event_reference` follows the same shape as `u32 hot_reference` and `u32
cold_reference`, so it composes into a `u64 action_reference` (the `server_reference:16 |
payload:32` word). Holding it as a `u32` means an action that carries an `event_reference` (e.g.
`AWAIT` on an aliased row) is passed **exactly like any other reference** — no special case.

**Why not `u64`:** a `u64` `event_reference` would force actions that carry one to be handled
*differently* from actions carrying hot/cold references, and would change how we pass the vector
of actions in an event log. That special-casing is the cost we're avoiding.

**Status:** design-decided; the code still keys `event_log` on a `u64` PK — reconciling it to
`u32` is a [`plan/`](../plan/) item (not blocked). See
[`current/README.md`](../current/README.md).
