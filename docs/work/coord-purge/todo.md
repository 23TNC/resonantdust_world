# Todo — coord-purge

_Ordered by dependency: **A gates**; B–E are independent safe deletions; **F/G** are the substantive
reworks (F browser-verified). Move an item to `remaining.md` when you start it, `completed.md` when it
lands. **A–E are done — see [`completed.md`](completed.md).** F and G remain (the substantive reworks)._

---

## F · Rework the origin to macro — the browser-verified purge

Thread `macro_position_reference` (u16) instead of `zone_id` through the whole cold + render path:

- **worldgen:** `zone_cold(macro)` (was `zone_id`); origin via `macro_world_origin` (item A), not
  `zone_world_origin`. Edge `seed_zone` passes the macro straight through (no `zone << 8`).
- **wasm prims:** `zone_tile_prims` / `zone_cold_prims` / `mover_prim` take `macro`, origin via
  `macro_world_origin`; drop `zone_origin`/`global_tile`.
- **events + client:** `Event::ColdTiles`/`ColdThings`/`StateObject` and their `ServerMsg` mirrors carry
  `macro_position_reference` (u16), not `zone_id`. pixijs keys + calls prims by macro. **Delete
  `world.rs::macro_to_zone_id`** (its only purpose was reconstructing `zone_id` for the render).

**Done when:** no `zone_id` in worldgen/wasm-prims/cold-events/pixijs-render; all four build gates
green; **and terrain re-checked in the browser — biomes still flow across zones** (the seam property).

---

## G · Rework the anchor manager to `macro_position_reference`

`zones.rs` (the 657-line hysteresis/LRU anchor manager) keys everything on the legacy `zone_id` (u32)
via `pack_zone_id`/`zone_at`. Rework it to key on **`macro_position_reference` (u16)**:

- `ZoneIntent` / `subs` / `coldRowsRaw` etc. keyed by `u16` macro.
- world tile → global zone (`div_euclid(ZONE_DIM)`) → `(region_x, zone_x)` nibbles → macro, all object
  math; no `pack_zone_id`, no realm/reserved.
- Update its unit tests to the macro keys.

**Done when:** `zones.rs` holds no `zone_id`/`pack_zone_id`; its tests pass; `rd build core` (native +
wasm) green. After this, `world.rs::zone_id_to_macro` (anchor→wire) is also gone — the manager already
speaks macro.
