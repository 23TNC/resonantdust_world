# Todo — coord-purge

_Ordered by dependency: **A gates**; B–E are independent safe deletions; **F/G** are the substantive
reworks (F browser-verified). Move an item to `remaining.md` when you start it, `completed.md` when it
lands. **A–F are done — see [`completed.md`](completed.md).** F's browser pixel-confirm is pending a
user dev-loop refresh (behavior-preserving by construction; see completed.md). G remains._

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
