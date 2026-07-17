# Todo — coord-purge

_Ordered by dependency: **A gates**; B–E are independent safe deletions; **F/G** are the substantive
reworks (F browser-verified). Move an item to `remaining.md` when you start it, `completed.md` when it
lands. **A, B, C, D are done — see [`completed.md`](completed.md).**_

---

## E · Fix the two latent transpositions in wasm

`free_thing_prim` and `mover_prim` still decode the in-zone cell with `packed::cell_x/cell_y` (the
transposing legacy convention — the same class as the ground bug). Neither is on the active path today
(`mover_prim` is always called with `location = 0`; `free_thing_prim` is unused), so this is
pre-emptive.

- If a function is dead (`free_thing_prim`?), **delete it**. Otherwise switch its cell decode to the
  canonical `object::ref_hi/ref_lo` (what `zone_tile_prims` + the things layer now use).

**Done when:** no `packed::cell_x/cell_y` left in `shared/wasm`; `rd build shared` green.

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
