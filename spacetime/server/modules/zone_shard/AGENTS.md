# AGENTS.md — `zone_shard` module (the hot/cold zone store)

## Purpose
SpacetimeDB 2.1.0 server module (Rust → wasm32), package `resonantdust_zone_shard`.
The new game's **zone shard**: the location-dependent store holding terrain plus
the things that have **settled into** the world. SpacetimeDB is the generic data
store, the **Gateway** is the game server. The zone shard holds two
representations of every **16×16 zone** and trusts the Gateway to validate writes
and to overlay the two representations when serving the client. Depends on the
shared `resonantdust-codec` crate (the `valid_at` / `zone_id` / packed-thing/tile
bit helpers), reached via the `../shared` bind-mount.

Mobile things and pawns live in a separate **`object_shard`** (planned): something
dropped into the world starts in an object shard, and becomes part of a region
shard once it settles. "Which shard holds a region" is the index's routing job —
this module treats `zone_id` as opaque.

Geometry: a **zone** is 16×16 tiles (256 cells; `location = y << 4 | x`, `0..256`).
A **region** is 16×16 zones (256×256 tiles). Region/zone composition into
`zone_id` is the **Gateway's** concern.
Every cell has two layers: **tile** (floor/wall) and **thing**. There is one thing
list per zone; a layer field carved from a packed thing's reserved bits will later
distinguish floor- from wall-level things.

Pawns are a later, separate model — they need instance identity the positional
tile/thing tables don't carry.

## Hot / cold
- **Cold** (`cold_zones`, [zones.rs](src/zones.rs)) — one settled row per zone:
  the full 256-tile terrain + the packed thing list. Big, rarely rewritten.
- **Hot** (`hot_tiles` / `hot_things`, [hot.rs](src/hot.rs)) — one tiny row per
  *changed* cell. Editing a cell fans out a ~16-byte row, not the whole cold zone.
- **The Gateway overlays hot atop cold** when serving the client. The shard just
  stores both; it never computes the merge.
- **GC fold** ([gc.rs](src/gc.rs)) folds at-rest hot cells back into **one** new
  cold row per zone (batched → minimal fan-out), back-dated, then deletes them.

## Module map
| File | Role |
| --- | --- |
| [src/zones.rs](src/zones.rs) | `cold_zones` table `{valid_at PK, zone_id idx, tiles: Vec<u16>(256), things: Vec<u32>}` + its hand-written `valid_at` primitives (`prior_at`/`latest`/`baseline`/`delete_at`/`write_at`/`reap_prior`), the `upsert_thing` fold helper, and the `seed_cold_zone` reducer. One cold table → no macro. |
| [src/hot.rs](src/hot.rs) | The two hot layer structs (`HotTile`/`HotThing`, identical: `{valid_at PK, zone_id idx, location: u8, rotation: u8, id: u16}`) + `decl_cell_history!` which generates each layer's primitives keyed on `(zone_id, location)` (`prior_at`/`latest`/`delete_at`/`delete_all`/`write_at`/`cells_in_zone`/`reap_prior`). The `set_hot` (single cell) + `apply_hot` (batched, one commit per zone) reducers + the `LAYER_*` dispatch. |
| [src/gc.rs](src/gc.rs) | `gc_schedule` + `init` (seeds the 10-min schedule) + `gc_sweep` → `fold_hot_to_cold` then prior-version reaps for all tables. `FOLD_HORIZON_MS` (30s) gates "at rest". |
| [src/time.rs](src/time.rs) | Server time source (`now_ms`) for the `valid_at` model. The old drift-grace contract was retired with the event-log sync model — reducers stamp at server time; the render delay is client-side (see `docs/sync.md`). |
| [src/sequence.rs](src/sequence.rs) | `sequence_counter` + `next_sequence()` — load-bearing for `valid_at` PK uniqueness across same-ms writes. |

The `valid_at` PK + zone-cell + packed-thing/tile bit helpers live in the shared
`resonantdust_codec::packed` crate (not in this module), so the zone shard and
the client encode the same layouts.

## Anti-drift: one history-primitive set, macro-generated
The previous iteration's cardinal sin (its own AGENTS.md flagged it) was a second
module keeping a *"drifted partial copy of the write primitives."* SpacetimeDB's
index accessors (`ctx.db.hot_tiles().zone_id()`) are concrete methods, not a
generic trait, so one generic `fn` can't span the tables. `decl_cell_history!` in
[hot.rs](src/hot.rs) generates each hot layer's primitives, so both are
bit-identical by construction. (The `#[table]` proc-macro does **not** expand
cleanly from inside a `macro_rules!` metavariable — generates a stray
`_table_name` reference — so the *structs* are written out explicitly; only the
*logic* is generated.)

## The `valid_at` pattern
```
valid_at: u64 = (time_ms: u48 << 16) | sequence: u16
```
- **`valid_at` is the primary key**; the logical id is a btree column —
  `zone_id` for cold, `(zone_id, location)` for hot (one btree on `zone_id`,
  `location` filtered in the closure; ≤256 cells/zone so it's cheap).
- **Time is milliseconds.** `now_ms(ctx) = ctx.timestamp.to_micros_since_unix_epoch() / 1_000`.
- **Same-(id, time_ms) writes are purged before insert** — `write_at` deletes any
  row at that exact key first ("last write at this (id, ms) wins").
- **Future-stamped rows are first-class.** `latest()` filters `valid_at_time ≤ now`;
  `prior_at(..., time_ms)` is the form every writer past `now` must use.

## Def ids: u12 everywhere (two namespaces)
Tile defs and thing defs are **separate namespaces** but share one **u12** width
(`packed::DEF_ID_MAX` = 4095), disambiguated by layer:
- **Packed thing** (`u32`, cold `things`): `x:4 | y:4 | rotation:2 | object_id:12 |
  reserved:10` (a layer field will later be carved from the reserved bits).
- **Tile slot** (`u16`, cold `tiles[location]`): `def_id:12 | reserved:4` (see
  `packed::pack_tile` / `tile_def` / `tile_reserved`).

The reserved bits in both are headroom for later. The hot `id` column is a u16 for
both layers (no u12 scalar exists), so **every** hot write is rejected when
`id > 4095` rather than truncating on fold. `def_id`/`object_id == 0` = empty.
Rotation is only meaningful for the thing layer (tiles don't rotate). The hot
tile row carries only the def, so the fold **preserves a cell's existing reserved
nibble** across a def change.

## The GC fold (anti-flash discipline)
`fold_hot_to_cold` reads each zone's cold baseline, applies its at-rest hot cells
(tiles → `tiles[loc] = id`; things → `upsert_thing`), writes **one** new cold row,
deletes the folded hot rows. The new cold row is **back-dated** to the folded
cells' own timestamps (clamped ≥ the baseline's time so it stays the latest cold
version) so it's promotable the instant the hot rows vanish. Stamping at `now`
instead would reintroduce the old "fold-back flash" (cell snaps to the stale
pre-change baseline for a buffer-length). Cells touched within `FOLD_HORIZON_MS`
stay hot.

## Write surface (Gateway-facing reducers)
The Gateway is the authority; the shard trusts its args (authorization is the
Gateway's job — same posture as the old `apply_action`). Each reducer takes the
Gateway-resolved `now_ms: u64`.
- `set_hot(now_ms, layer, zone_id, location, rotation, id)` — single cell.
- `apply_hot(now_ms, layer, zone_id, locations[], rotations[], ids[])` — batched,
  one commit per zone (parallel JSON arrays over `/call`).
- `seed_cold_zone(now_ms, zone_id, tiles[256], things[])` — worldgen / bulk load.

`layer`: `0 = tile`, `1 = thing` (`hot::LAYER_TILE` / `hot::LAYER_THING`).

## Build & iterate
Builds to wasm inside Docker — never a host `spacetime` CLI.
- `bin/rd build spacetime zone_shard` — module wasm + regenerates server bindings
  into `server/src/bindings/zone_shard/`.
- Iteration-only compile check (skips bindings):
  `docker compose -f spacetime/compose.yml run --rm -w /workspace/server/modules/zone_shard build build`.
- `rustfmt` is absent in the image — the "could not format" warning is cosmetic.

## SpacetimeDB 2.x notes
- `#[table(accessor = cold_zones, public)]` — `accessor = <ident>` is the 2.x form.
  Scheduled tables: `#[table(accessor = …, scheduled(<reducer_ident>))]` + a
  `scheduled_at: ScheduleAt` column.
- `Vec<T>` columns (e.g. `Vec<u16>`, `Vec<u32>`) are supported — confirmed
  building. (The old code packed into flat `u64` columns; not needed here.)
- Calling another module's accessor needs the trait in scope: `use crate::hot::hot_tiles;`.
- `#[auto_inc]` only on the `#[primary_key]` column.
- Private tables (no `public`) are skipped during bindings codegen.

## Pitfalls
- **Don't hand-roll `valid_at`** — the `write_at` helpers stamp it via
  `pack_valid_at(time_ms, next_sequence(ctx))`.
- **Use `prior_at`, not `latest`, when writing past `now`.**
- **Fold writes back-dated** — never stamp the folded cold row at `now`.
- **`tiles` must be length 256** — `seed_cold_zone` rejects otherwise; use
  `zones::empty_tiles()` for a blank zone.
