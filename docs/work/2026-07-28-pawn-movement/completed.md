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

## 2026-07-28 · P3 — kill the snap-tween-snap (5/5)

**Baseline** (honest note: the planned structured 10-trip pre-fix count was preempted — vite
hot-reloaded the page the moment the fixes landed — so the baseline is the captured evidence
from the pre-fix sessions): arm `d` climbing 5.5 → 16.1 over ~35 s in one session and
27.7 → 35.0 in the next (the ratchet), landings at e=10.00 / 7.00 / 5.00 / 4.00 tiles, the
same intents delivered TWICE at 9:25:49, and stale replays with old-epoch tics
(39682/24398/1626/732 seen at the npc). Root causes then measured: **the durable tic runs at
5.41 Hz, not 6** (541 tics / 100.04 s — I5), which the old constant-rate max-anchor estimator
turns into an unbounded lead (I4).

**Fixes landed**: (1) `TicEstimate` rewritten (F6) — learned rate (two-point over the 10 s
re-anchor window, clamped 0.5–1.5×`TIC_HZ`), windowed re-anchor so freshness beats the max
rule beyond the delivery-delay bound, poison band ±900 tics with an 8-streak hard reset;
`TicAnchor` carries `tics_per_sec`, hosts extrapolate with it; 5/5 ticclock unit tests
(drift-bounded lead < 10 tics with learned rate within 0.4 of true; ahead-replay rejected;
poisoned cold anchor self-heals). (2) MoverLayer `pendingIntents` — an intent for an unseen
mover or an unanchored clock is HELD and re-evaluated each `tick()`, never dropped; cleared on
remove/zone-close. (3) `event_shard::gc` retention reducer + master call on the gc cadence
(`tic − 64`, every 20); st-bindings + edge bindings regenerated; module republished.

**Verified live** (2.9-min soak, fresh page): **14 intents armed / 12 landings (2 in flight)
— every trip armed EXACTLY one spec, none absent, none doubled** (the I3 acceptance);
arm `d` FLAT at 3.8–5.0 tics across the whole soak (≈ true settle+fan latency; the ratchet is
dead); landings converge to e = 0.00–0.22 tiles once the rate learns (~60 s; the first two
trips land at e=1.38/0.90 while the seeded 6 Hz converges to ~5.4). `event` table bounded at
2 rows under continuous trips (was: all history), zero "event gc failed" in master logs, zero
stale arms on the fresh page. The stale-binary guard from sim-self-heal fired on the npc
during this phase (client core had changed) — rebuilt instead of running stale.

## 2026-07-28 · P4 — authoritative snap + wrap (3/3)

Authoritative-override confirmed end-to-end on the soak: interim authoritative rows RESEED the
spec (`spec reseed e=…` lines), the landing CLEARS it (`spec landed e=…`), every correction
logs its error — the F8 path intact; the tween-blend recorded in `ACTIONS.md` §Movement as
the second held knob (F4), and the §Movement tic-estimate paragraph updated to the learned
rate. Browser acceptance: 12 consecutive trips gliding at 2 s/tile with no teleports beyond
authoritative snaps (trip walls match hops: 5 hops ≈ 8 s = seed + 4×12 tics); screenshot
ss_8728f2o1t — the wolf mid-glide at the focus tile among lit conifers. Memory + index
updated; stream closed.
