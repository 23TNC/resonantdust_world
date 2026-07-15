# Notes — variables

Supporting material for [`../VARIABLES.md`](../VARIABLES.md), which is authoritative and carries the
shapes alone. Nothing here defines a layout. If the two disagree, VARIABLES.md wins.

Deeper reasoning for the codec's types (the definition/position/data split, the cold/hot boundary):
[`../components/shared/codec/design/reference-model.md`](../components/shared/codec/design/reference-model.md).

## Why a single registry

These names cross the codec, the spacetime modules, the edge, the DSL and the client. A name — or a
nibble order — that drifts between any two is a live, silent bug. Precedent: an early sketch had
`layer_reference` with its nibbles reversed and gave `hot_reference` a positional layout; both would
have corrupted every decode.

## Changing a layout

Edit VARIABLES.md first, then conform the code and any doc reproducing it. A layout change is a
wire/storage break — everything that packs or unpacks it moves together.

Code of record: `shared/codec/src/{object,refs,packed}.rs`. It is *of record*, not the source of
truth; when it drifts, conform the code.

## Identity — why it's shaped this way

**The reference carries no type tag; the server does.** A server serves objects of one `type_id`,
and that nibble is the high half of its `server_reference`. So an `entity_reference` says what it is
by saying where it lives, and needs no variant tag of its own.

**Not realm-unique, on purpose.** Realm is a functional unit — every server in one works together.
`(server_reference, object_reference)` is `(who minted it, its counter)`, unique within a realm
because a server only mints its own ids. Only traffic that leaves a realm pays the extra 8 bits, via
`realm_server_reference`. The old model kept realm in every reference everywhere to serve the rare
cross-realm case.

**Server allocation: hand out `server_reference`s in increments of 16** — walk the u8 by 0x10 (0x10,
0x20, 0x30 …). That holds `server_id` at 0 and advances `type_id`, so servers spread across the type
space instead of piling into one type. A second server for a type takes `server_id = 1` (0x11, 0x21
…). Non-data servers (gateway, edge) may collide with these ids; they don't address objects, so the
shared id space costs nothing.

**A position is not an identity.** 32 bits of geography don't fit in a 24-bit `object_reference`,
and don't belong there. A position says *where*; a reference says *which*.

## Why `tic` is a wrapping u16

A ring, compared by serial arithmetic (RFC 1982) — the same "low numbers can be above high numbers"
trick the shadow work uses for angles. `shared/codec/src/tic.rs`; every comparison in the pipeline
design (`event_tic <= now`, `lease_tic < now`, the read rule's `t <= h.tic`) goes through it.

**Why not u8.** Not causality — nothing is implemented, and the guards all sit inside the runway
anyway (`TIC_GAP` = 3, `CLAIM_LEASE_TICS` = 4). The constraint is **scheduling runway**: a u8's
usable half-window is 127 tics ≈ 60s at 2 Hz, ≈ 30s at 4 Hz. That's not enough room to schedule
meaningfully into the future. A u16 gives 32767 tics — ~4.5 h at 2 Hz, ~2.3 h at 4 Hz — against a
scheduling need of a few tics.

**Why not u32.** It never wraps (68 years at 2 Hz), but the ring is cheap once implemented and the
bytes are on every `state_log` / `state` / `event_log` row.

**The one real limit.** Ordering holds only within `TIC_WINDOW` (32767). Two tics further apart than
that compare *wrong* — not "unknown", wrong. Bounded by construction for `event_tic` and `lease_tic`
(both `master + <5`) and for `state_log.tic` (GC horizon). The exposure is `state.tic` on a
long-idle object: a rock untouched for >4.5 h carries a tic that no longer orders against
`master_tic`. Nothing compares it today; if something ever needs to, it needs an epoch, not a wider
tic. `tic.rs` pins this failure mode in a test so it can't drift silently.

**`event.event_tic` wrapping is fine** — the log's total order is `event_reference` (ascending, and
globally total because the server byte makes shards disjoint), not the tic. The tic is a scheduling
label, not history's spine.

## Removed

### `valid_at : u64` — 2026-07-15

Was `time_ms:48 | sequence:16`, the primary key of the bitemporal model: rows for one key ordered by
the composite, `sequence` tie-breaking same-millisecond writes.

Both users stopped needing it. `players` was a version-history table whose history a GC sweep reaped
every 10 minutes and nothing ever read — flattened to one row per player, keyed by `player_id`
(which also let `name` become schema-`#[unique]`, previously impossible because a player's own
version rows collided on it). `chat_messages` only borrowed the shape for same-millisecond collision
avoidance — now an `auto_inc` `message_id` plus a plain `sent_at_ms`.

The go-forward ordering model is **tic-based** (`../intent/spacetime-again/`), not bitemporal. If you
need per-row time, use a plain ms column.

### `cold_reference` — 2026-07-15

A second type over `position_reference`'s 32 bits, meaning "the settled object here, unpack it". It
addressed an object by its coordinates, conflating place with identity, and it forced the tagged
union that `reference_id` existed to discriminate. Cold *storage* is unaffected —
`cold_row_reference`, `kind_pos_reference` and `data` are unchanged.

### `hot_reference` → `object_reference` — 2026-07-15

`hot_reference` was the opaque minted id, one variant of an `object_reference` union. With the other
variants gone there is no union: what `hot_reference` was **is** `object_reference`. Narrowed u32 →
u24 to fit `entity_reference` in a u32.

### `reference_id : u6` — 2026-07-15

The variant tag (`REF_NONE/HOT/COLD/POSITION/EVENT/SERVER`) on an `entity_reference`. With one
reference type left, there is nothing to tag. Type now comes from the server.

`event_reference` survived the collapse as an **alias**, not a variant: an event is an object like
any other, minted by an event shard, so its `entity_reference` carries `TYPE_EVENT` in the server's
`type_id` nibble. It briefly went missing from VARIABLES when the variants table was deleted —
`REF_EVENT` was a tag, the alias is not.

### `event_word : u64` — 2026-07-15

`op_code:4 | reserved:12 | server_reference:16 | payload:32` — the wire unit of the tick pipeline's
`actions: Vec<u64>` RPN program, with `OP_LITERAL/OBJECT/ACTION/ALIAS` and an `ACTION_*` palette.

Deleted because it had **zero call sites**. Its only consumers were `shared/tick`'s vm
(`encode_move`/`encode_spawn`), the worker, and the edge's move/spawn handlers — all removed with the
shard. It outlived them only because it lived in `shared/codec`, which survived, so a sweep for
dangling crate references didn't catch it.

The rebuild still wants an RPN `actions: Vec<u64>` but has not specified a word format. That's its
call to make, not a shape to inherit. Recover the old one from
`git show checkpoint/pre-shard-rebuild:shared/codec/src/event_word.rs`.

### `entity_reference : u64` → `u32` — 2026-07-15

Was `reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`, where
`server_reference` was `realm_id:8 | server_id:8`. Halved by dropping the variant tag, dropping realm
out of the common path, and narrowing the handle to u24.

### `players.data_shard` → `player_shard_reference` — 2026-07-15

Same width, but a reference rather than a bare partition index. `data_shard`'s `0` meant "the one
card shard" — it named nothing and couldn't address a shard in another realm.

### `data`'s `sub_position` and `aux` — 2026-07-15

`data` was `sub_position:3 | rotation:2 | aux:3`. `sub_position` (8 offsets internal to the tile) is
gone — a cold object sits on its tile. `aux` became `count`, named for what it holds and widened 3→6
bits (max 7 → 63) with the freed bits.
