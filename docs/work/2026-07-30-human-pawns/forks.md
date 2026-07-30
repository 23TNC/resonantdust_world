# Forks — human-pawns

## F1 (plan-time) — variants ride a widened pawn `data:u16`, not the def

**Chosen:** `entity_tables!(data: u16)` for the pawn shard only:
`body:4 | head:4 | facing:2 | serial:6` (low byte unchanged).
**Rejected:** (a) `definition_reference.variant_id` — a pawn's stored def is today a raw
content `object_id`, not a packed def; packing it is the object-model migration, a bigger
stream, and it holds only ONE u4 anyway. (b) squatting in the existing u8 — it is fully
occupied by `facing:2 | serial:6` (MOVE_STEP supersession reads the serial). (c) a new
column — a macro-shape change touching every `entity_tables!` consumer for one shard's
need. The README records that body_variant migrates into `def.variant_id` when pawn defs
become packed defs.

## F2 (plan-time) — head variants arrive via `CREATE` arity 3, not a follow-up verb

**Chosen:** `CREATE def · position · data` (imm). **Rejected:** (a) `SET` on the minted
id — SET targets cold rows only; (b) a new pawn-data verb — the client never knows the
minted id at issue time, so nothing can target it; the spawn transaction is the one place
the full first row is known.

## F3 (plan-time) — variant + part are stem segments, not atlas cells

**Chosen:** stem grammar `<base>/<variant>/<facing>.<part>` (defaults `1` / `0`), each
combination its own served master + own immutable defs, fetched on demand.
**Rejected:** packing variants into per-facing atlas grids (the linked-wall cell shape) —
would require an ingest step regenerating atlases whenever variant counts change, and the
`cell` lane is already claimed by linked-neighbour context; per-variant leaves are what
the go-forward tree (`texture-layout-folder`) already stores on disk.

## F4 (plan-time) — part index selects the state nibble

**Chosen:** part 0 ⇒ `body_variant`, part 1 ⇒ `head_variant` — positional, matching the
user's "part 0 is body, part 1 is head"; no new DSL selector field. **Rejected:** a
`&prim.variant_source` enum — more author surface for a mapping that is definitionally
positional today; revisit only when a part exists whose variant is NOT its own nibble
(hands may share the body nibble — that decision belongs to the hands stream).
