# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

## Retire the legacy world-global `zone_id` (geometry cleanup — UNBLOCKED)

The identity/event re-cut **and** the object-model re-cut (`object.rs` → reference model) are
**done + browser-verified** ([completed.md](completed.md), 2026-07-14). What remains is retiring the
one acknowledged legacy layer: `packed.rs`'s **world-global `zone_id : u32 = region_x:8 |
region_y:8 | surface:8 | zone_x:4 | zone_y:4`** — from the old game (it carries a dead **surface**
byte and 256-wide regions instead of the go-forward **realm · region · zone · tile · layer**).
`spatial-references.md` itself frames it as "the legacy routing key it will reconcile against." Not
blocked — normal work.

- **2026-07-14** · **Repartition `zone_id` → geographic + drop `surface`** — `zone_id : u32 = realm:8
  | region:8 | zone:8 | reserved:8` (each level `hi:4|lo:4`; drops surface, adds realm). Touches
  `packed.rs` (`pack_zone_id`/`zone_and_location`/`global_tile`/`region_of`), `biome.rs`
  (`zone_world_origin`), `wasm` js wrappers (`pack_zone_id_js`, drop `zone_surface_js`,
  `zone_origin`), `client/core` (`zones.rs`/`engine.rs` `set_anchor` — drop the `surface` param or
  make it `realm`), `pixijs` (`setAnchor` call), edge/index `region_of` routing. Realm→shard
  routing; the demo stays in realm 0 / region 0.
- **2026-07-14** · **#4 `region_zone` cold key** — move the `cold` table from `zone_id:u32` to
  `region_zone_reference:u16` (realm implied by the shard); carry it on the wire (`ColdObjectsRow`,
  `SubZone`) + the client demux. Switch the cold `entity_reference` from the interim world-global
  `zone_id|location|layer` form to the geographic `REF_COLD | server_reference | cold_reference:32`
  (`object.rs` `pack_cold_reference` already exists). Retire the `refs.rs` interim cold packing.
- **2026-07-14** · **`PACK` trigger** — `pack_settle` exists; wire *who* calls it + the
  pack-criterion (`data:8` fits ⇒ packable, now that `data:8` is defined). (No live trigger in the
  current demo: pawns never pack.)

## Not blocked (small, deferrable)

- **2026-07-14** · **Cross-shard foreign Phase-1 hold** — a foreign target gets no pending
  row/holder on its home shard during the in-flight window (read-rule/GC visibility). The
  convergent *write* is done ([completed.md](completed.md) #4); this is the in-flight *hold*.
  Eventually-consistent today; low priority.
