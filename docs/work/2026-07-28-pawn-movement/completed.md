# Completed — pawn-movement

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-28 · P1 — speed is content (3/3)

Docs first: `ACTIONS.md` §Movement + `codec::speed` module docs now state the tics-per-tile
authoring rule (F1, supersedes wall-time) with the accepted `TIC_HZ` consequence; docs-check
green, no authoritative doc claims wall-time authoring (first-pawns folder entries are history,
left alone). DSL: `12 &thing.speed set` in the wolf's `:data @define`
(`content/data/things.rd`); loader `thing_speed(object_id) -> Option<u16>` +
`thing_speeds()` bundle table — unit test `thing_speed_from_data_define` green (wolf → 12,
unauthored → `None`, table `[0.0, 12.0]`). Client bundle: `thingSpeed` wasm export; verified
in the live browser console via the new `__content` debug global (mirrors `__client`):
`thingSpeed()` = `[0,0,0,0,0,0,12,0,0]` — index 6 = object_id 7 = wolf.

## 2026-07-28 · P2 — every consumer reads the SAME speed (4/4)

`codec::speed` is now `DEFAULT_TICS_PER_TILE = 3` (PINNED in tics, not derived from `TIC_HZ` —
deriving would smuggle wall-time back in through the default) + `resolve(Option<u16>)` clamped
≥ 1; the def-taking stub is DELETED and the compiler found every caller (52 codec tests green).
worker: corpus loaded from disk at startup (`CONTENT_DIR` default `/workspace/content`,
edge-`load_disk` file order — F3; missing corpus = exit for the restart policy, running would
drift), continuation spacing per def. **Verified live: composed components at tic
8394 → 8406 → 8418 → 8430 — exact 12-tic hops, 2 s apart**; startup logs "corpus speeds
loaded kinds=9". npc: `resolve_thing` returns `(def, speed)`; logs "wolf def + speed resolved
def=7 speed=12"; multi-trip soak shows every trip arriving inside its deadline (4 hops ≈ 7 s,
2 hops ≈ 3 s — seed hop at the queue tic + (N−1)×12), zero premature re-issues. webgl:
MoverLayer `speedFor(kind)` off the bundle table (`defaultTicsPerTile` fallback), refreshed on
hot-swap; live trips arm and land at e=0.01 / 0.02 tiles. FINDING for P3: intents arm with
`d ≈ 28` — the wall↔tic estimate leads the server by ~25 tics, so the spec starts ~2 tiles
along before gliding (landing tics themselves are exact: 9102 − 9042 = 60 = 5 hops × 12);
recorded as issue I4.
