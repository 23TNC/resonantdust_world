# Forks — human-pawns

## F1 — per-pawn state is a growable opcode stream, not packed nibbles (USER, 2026-07-30-b)

**Chosen (user-directed):** pawn rows gain `payload: Vec<u32>` encoded like the command
buffer — `opcode:16 | count:16` header + `count` operands per entry; first opcode
`PART (slot, definition_reference)`. Pawn defs repack to real
`TYPE_PAWN | species | kind | variant` defs; the wolf's variant is `def.variant_id`;
`data:u8` (`facing:2 | serial:6`) is untouched.
**Superseded (my plan-time draft):** widening `data` to u16 with fixed
`body:4 | head:4` nibbles. The user's counter: pawns will accrue inventory, stats, needs,
memories — a nibble per feature dead-ends immediately, while an opcode stream
grows/shrinks per pawn (a wolf gaining an inventory is one new entry, no schema change),
and full defs per part slot are what let armor SWAP a slot's art later. The draft's sole
advantage (no def repack) was debt, not economy.

## F2 — the payload arrives via variable-arity `CREATE`, not a follow-up verb

**Chosen:** `CREATE def · position · count · payload×count` (the `INIT_ZONE` variable-arity
precedent). **Rejected:** (a) `SET` on the minted id — SET targets cold rows only; (b) a
new pawn-payload verb at spawn time — the client never knows the minted id at issue time;
the spawn transaction is the one place the full first row is known. (A LATER equip verb
targeting a known id is expected and out of scope.)

**Extended (USER, 2026-07-30-c):** `CREATE` is THE generic creation verb for essentially
any object (a pawn, a stack of logs) — no new `spawn` action. The packed def's `type_id`
becomes the routing key to the type's shard arm; this stream implements `TYPE_PAWN` only
and REJECTS other types by name (never silently pawns them), so future arms slot in
without reshaping the verb. `CREATE` mints identity + first row; `SET` stays the
no-identity cell write.

## F5 — the payload lives in sidecar tables SLAVED to state (USER, 2026-07-30-d)

**Chosen (user-directed):** two more pawn-shard tables, `payload_log` + `payload`
(log/composed split), leaving the generic entity rows alone — hops cheap, payloads
complex. The cross-table lock hazard (payload dirty/locked ⇒ can't edit state; state
locked ⇒ can't edit payload; an action touching one while the other is claimed breaks) is
resolved by SLAVING: the payload is never claimed independently — the entity's state
claim IS its lock, every payload write rides a state-write transaction (spawn now, equip
later), and the existing lock machinery runs unmodified. Zone key: the `payload` row
carries it for the zone subscription; only a zone-crossing state write re-keys it.
**Superseded (the -b draft):** `payload` as a column on the entity rows — every movement
hop would copy the vec forward, ruinous once inventories exist (the README's flagged
PACK-normalization concern, now resolved structurally instead of deferred).

## F3 (plan-time) — variant + part are stem segments, not atlas cells

**Chosen:** stem grammar `<base>/<variant>/<facing>.<part>` (defaults `1` / `0`), each
combination its own served master + own immutable defs, fetched on demand.
**Rejected:** packing variants into per-facing atlas grids (the linked-wall cell shape) —
would require an ingest step regenerating atlases whenever variant counts change, and the
`cell` lane is already claimed by linked-neighbour context; per-variant leaves are what
the go-forward tree (`texture-layout-folder`) already stores on disk.

## F4 (plan-time) — the DSL declares slots; the payload dresses them

**Chosen:** the DSL kind's prims are the SKELETON (slot index, `part` files, `scale`,
`offset`); a payload `PART(slot, def)` entry supplies what the slot draws; a slot with no
entry draws the pawn's own def (the wolf's whole model — "the easy case of wolf can just
accept variant"). **Rejected:** variants-by-position in state nibbles (superseded with F1)
and a `&prim.variant_source` DSL selector — the payload def IS the selector, and it
already carries its u4 variant.
