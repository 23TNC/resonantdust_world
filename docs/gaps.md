# Simulation pipeline — known gaps & planned work

Status snapshot after the A/B/C build (see [`simulation.md`](simulation.md) for the
design, [`simulation-plan.md`](simulation-plan.md) for the build history). The engine
(A0–A4), browser integration (B), and the distributed layer (C1–C3, cross-shard reads +
object↔zone migration) are built and proven live. This is what's **not** done.

## Correctness / robustness gaps

These can bite under real (not demo) conditions and should be closed before the pipeline
is load-bearing.

1. **Master catch-up for a lagging shard.** `server_master` lockstep-bumps each shard
   `+1` per interval; a shard that missed a bump (network/overload) stays permanently
   behind and never converges, which makes a cross-shard read of an actor on that shard
   read a *stale* (earlier-tic) value. Plan: make `bump(target)` loop master up to a
   shared global `G` (a lagging shard closes its gap in one call), **and** add a
   read-level guard — a cross-shard actor read *blocks* if the actor's shard `master_tic
   < read_tic`, so drift can never yield a stale read, only a wait.

2. **Shard-id assignment not wired into deploy.** The object_id uniqueness guarantee
   depends on every data shard having a **distinct, permanent** `shard_id` and never
   booting as `SHARD_NONE`. Today `set_shard_id` is a manual reducer call. It must be
   owned by the deploy path (`bin/rd index seed` assigning each data shard its id from
   the routing config), and a shard must refuse to mint while unset.

3. **Transfer saga hardening.** The C3 migration works but is minimal:
   - the initial **transfer-flag hop** (mark the source in-transit before `receive`, so
     it can't be mutated mid-migration) is not implemented;
   - removal is a **`kind==0` tombstone**, not a true row deletion (dead rows linger
     until GC could reap them — GC currently keeps latest-per-entity, so a tombstone is
     retained);
   - **one-transfer-per-tic** per object isn't enforced;
   - **cross-tic sequencing is manual** — the orchestrator (edge/driver) must space
     `receive` and `ack` a tic apart; nothing enforces it, and a same-tic pair would
     duplicate.

4. **`object_id` count ceiling.** `count` is 32 bits → ~4.3 B lifetime mints per shard,
   monotonic (deaths don't free it). Fine for a game, but a hard per-shard ceiling; 6
   spare `object_reference` bits + a trimmable `obj_type` give headroom if ever needed.

## Feature / model gaps

Capabilities designed but not built.

5. **Hot/cold world objects (pack / unpack / fold).** The plan for walls/floors/tiles/
   trees: cold packed baseline + hot per-cell tick entities + `unpack` (cold→hot),
   `pack`/fold (hot→cold), and object↔zone `transfer` (C3) for pickup/place. This is the
   path that moves the world map onto the pipeline — and the prerequisite for the legacy
   `hot_*`/`cold_zones` cleanup. (Design agreed; not built.)

6. **`u8` layer — multiple objects per tile.** **codec step DONE** — zone key is now
   `(zone_id, location, layer)` (`pack_zone_key`/`entity_layer`, layout
   `zone_id:32<<16 | location:8<<8 | layer:8`), so 256 entities can stack on one cell.
   Remaining (with the world-object phase): the client renders zone cells per-layer with
   z-order, and `pack`/attach picks the destination layer. Plan below.

7. **Client stale-object indication.** Deferred: gray-out entities whose per-object tic
   lags the visible frontier. The pipeline supports it (`state.tic`); the edge/client
   don't surface it yet.

8. **Backpressure / runaway guard.** The adaptive lag controller was dropped for the
   per-object-frontier model; only a coarse "skip a tic if master outruns the global min
   frontier" guard was noted — not implemented.

## Cleanup (mostly world-object-phase-entangled)

9. **Legacy tables.** `object_shard`: `free_things`/`pawns`/`debug_mover`/`presence`/
   `sequence`/`transfer`/`gc`. `zone_shard`: `cold_zones`/`hot_*`/`transfer`/`sequence`/
   `gc`. Can't delete `cold_zones`/`hot_*` until the world map moves onto the pipeline
   (gap #5) — the browser's terrain/thing rendering still rides on them.

10. **Edge rename** `server` → `server_edge` (deferred to last, churns bindings/compose).

11. **`tick_gc`** was named to dodge a collision with the legacy `gc.rs::gc_sweep`;
    reverts to a clean name once legacy `gc.rs` is deleted.

12. **Orphaned demo objects.** The object-id format change left old-format demo objects
    in `state` (the new client ignores them by `obj_type`); a `--delete-data` clears them.

## Smaller / deferred decisions

- Per-crate generated `bindings/` in master/simulation/npc (dedup into a shared crate later).
- `data[u64;2]` sufficiency for richer entity state (fine for hp + a few fields).
- Presence / region routing unexercised (single default `db_name` per class so far).

---

## Plan: `u8` layer for multiple objects per tile

**Problem.** A ZONE entity key encodes `(zone_id, location)` only, so a tile can hold one
location-keyed cell. Terrain needs to stack: a floor *and* a wall *and* affixed things
(a rug, a lamp) at the same cell. (Free/mobile things already stack — they live on the
object shard keyed by `object_id`, positioned by `(zone_id, location)` in their state, so
any number share a tile. The layer is only for the *location-keyed* zone cells.)

**Approach.** Add an 8-bit `layer` to the zone key, so `(zone_id, location, layer)` is the
identity. Distinct layers are distinct entities → up to 256 stacked cells per tile,
resolved independently by the existing pipeline (the key is opaque to resolution, so
**no reducer or read-rule change**).

### Steps

1. **codec — zone key gains `layer`. ✅ DONE.**
   - `pack_zone_key(zone_id, location, layer)`; layout `zone_id:32<<16 | location:8<<8 |
     layer:8` (48 bits, 14 spare); `entity_zone_id`/`entity_location`/`entity_layer`;
     `LAYER_FLOOR/WALL/THING`. Codec + tick tests green.

2. **Pipeline — no change.** Layer lives inside the opaque `entity_key`; different layers
   are different entities. Resolution, read-rule, priority, GC are untouched. Keep `layer`
   **in the key only** — no new `state`/`state_log` column (the pipeline is
   layer-agnostic, and the edge already forwards `entity_key`, so the client can decode
   the layer). SQL zone-filtering still works via the `zone_id` column. Add a column only
   if we later need to filter by layer in SQL.

3. **Client rendering.** Extend the `RowData::State` decode to handle ZONE entities:
   decode `(zone_id, location, layer)` from the key and draw affixed cells with
   layer-ordered z-index (floor under wall under things). Lands with the world-object
   rendering (gap #5) — mobile-object rendering (demo/player) is unaffected.

4. **Transfer interaction.** `unpack` (zone→object) drops the layer — a mobile object has
   no layer. `pack`/attach (object→zone) targets a specific layer, so the `receive` event
   carries the destination `layer` (which layer the object settles into). Slots into the
   C3 saga unchanged.

### Scope

Step 1 (codec) is small, self-contained, and can land now — it just widens the zone key
and updates the handful of `pack_zone_key` callers (codec + tick tests, the CLI test
producers). Steps 3–4 land with the hot/cold world-object phase (gap #5), which is where
layered cells are actually produced and rendered.
