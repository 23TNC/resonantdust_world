# Todo — spacetime-again

_Planned, not started. W1-W8 are done — see [`completed.md`](completed.md). Move an item to
`remaining.md` when you begin it. Ordered by dependency — each item's surface is what the next one
consumes. **W9 is last** — `shared/wasm` (the JS bridge), `client/npc` (drive pawns via the new
program/queue path), `client/pixijs` (render the movers). Then a wolf moves in the browser._

---

## W9 · `shared/wasm`, `client/npc`, `client/pixijs` — follow

**Components:** three, done **one at a time**, in this order — each consumes only the one before.

- `shared/wasm` — the JS bridge; `LoggedIn` already carries `playerShardReference`.
- `client/npc` — `pack_hot_entity` → `pack_entity_reference`; wolves need a `server_reference` with
  `TYPE_PAWN`, not `SERVER_REF_NONE`.
- `client/pixijs` — `MoverLayer` hardcodes `REF_HOT = 1` and filters on it. That constant is gone;
  the type is the server's `type_id` nibble.

**Done when:** `rd build shared`, `core --check`, `npc --check` and `tsc --noEmit` are all green —
the first time since the deletion — and a wolf moves in the browser.
