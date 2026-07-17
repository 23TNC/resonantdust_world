# Work — coord-purge (retire `zone_id` / legacy `packed` from the live paths)

_Opened 2026-07-17. **Complete 2026-07-17** — all items A–G landed; see [`completed.md`](completed.md).
Conforms the coordinate code to the object model in [`VARIABLES.md`](../../VARIABLES.md); no new design.
Follows [`world-storage`](../../intent/world-storage/README.md). One hand-off remains: F's browser
pixel-confirm on a dev-loop refresh (behaviour-preserving by construction — see completed.md / todo.md)._

## What & why

The go-forward geographic address is **`macro_position_reference`** (`region_reference:8 |
zone_reference:8`) + `tile_reference`, all in [`shared/codec/object`](../../VARIABLES.md). The legacy
**`zone_id`** (`u32 = realm:8 | region:8 | zone:8 | reserved:8`) is marked *superseded* in VARIABLES,
yet it still threads through the cold/render/worldgen paths and the anchor manager, dragging the
legacy `packed::` decoders with it.

That bleed is not cosmetic — **it is a bug generator.** The ground-tile seam bug (biomes shearing at
zone lines) was exactly this: worldgen wrote the dense index as a canonical `tile_reference`
(`tile_x` high nibble) while the renderer read it back with `packed::cell` (`tile_x` low nibble),
transposing every zone. Each remaining legacy call site is a latent copy of that class of mismatch.

## The invariant (done when)

**No `zone_id` and no legacy `packed` geo-helper anywhere in the cold, render, worldgen, or anchor
paths.** World position comes only from `macro_position_reference` / region·zone·tile. The world-size
constants live in VARIABLES, and the object-model code reads them from there.

## How this one runs

- **A gates everything** — it sets the authority (VARIABLES) the rest conforms to.
- **B–E are independent, safe deletions** (dead code + off-path fixes); do them in any order.
- **F and G are the two substantive reworks.** **F is the browser-verified one** — re-check terrain
  on `:5173`/`:5174` after it lands (the seam bug lived here). G is `zones.rs` internals, unit-tested.
- One item per commit, verified against its surface (build gate, unit test, or the browser for F).

## Left alone — recorded so it isn't re-litigated

- **Player routing stays live** — `index`'s `servers` / `player_servers` (`server_id`/`player_id` →
  edge) is how a client reaches an edge; not legacy, not touched. Only the *zone→data-shard* router
  (`region_shards`/`shards`, `resolve_zone_or_default`, `region_of`) is dead and removed (item C).
- **The multi-shard data router is rebuilt fresh, macro-based, when there's a real need** — not
  adapted from the old `zone_id`/`region_id` model. Until then it simply doesn't exist.
- **Realm stays 0 / unused.** The code carries realm as a level but never sets it; a bigger world
  comes from widening a reference (`u8 → u16`, `DIM 16 → 256`) or lighting up realms — later.

## Files

`todo` → `remaining` → `completed`. `forks`/`issues`/`blockers` appear if they gain content.
