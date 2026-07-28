# Todo — pawn-movement

_Items tick in place; the box is the move. Design: [`README`](README.md) ·
[`ACTIONS.md`](../../ACTIONS.md) §Movement · [`forks`](forks.md)._

---

## P1 · Speed is content (docs first)

- [x] Docs FIRST: rewrite the speed rule in `ACTIONS.md` §Movement + `shared/codec/src/speed.rs`
      module docs — speed is authored in **tics per tile** in the DSL corpus per kind
      (user decision, supersedes wall-time authoring — F1); state the accepted consequence
      (a `TIC_HZ` change now changes wall-clock speed; time is measured in tics). Acceptance:
      `bin/rd docs-check` green; no doc still claims wall-time authoring.
- [x] DSL: a thing's `:data` facet authors `speed` (u16 tics/tile) via the existing hook/param
      mechanism (pick the idiom the loader already uses — see F1 note); loader exposes
      `thing_speed(object_id) -> Option<u16>`; author the wolf at **12** in
      `content/data/things.rd`. Acceptance: loader unit test — wolf resolves 12, an unauthored
      kind resolves `None`.
- [x] Client bundle: a `thing_speed()` per-kind table beside `thing_layout()` (loader → wasm
      export → `Content`), refreshed on content hot-swap. Acceptance: browser console reads the
      wolf's speed 12 from the bundle.

## P2 · Every consumer reads the SAME speed

- [x] `codec::speed` becomes the DEFAULT + resolution rule only: `DEFAULT_TICS_PER_TILE`
      (compute from `TIC_HZ` once, or pin it — state which in the code docs) and
      `resolve(authored: Option<u16>) -> u16`; delete the def-ignoring stub signature so no
      caller can silently keep the old path. Acceptance: `cargo test` in codec; grep shows no
      caller passing a def id to codec.
- [x] worker: load the corpus at startup from disk (`CONTENT_DIR`, default `/workspace/content`,
      same file order as the edge's `read_content_dir` — F3); continuation spacing =
      `resolve(corpus.thing_speed(def))`. Acceptance: soak logs show wolf hops at exact 12-tic
      spacing.
- [x] npc: the wolves brain's trip deadline uses the corpus speed it already fetches (extend the
      `/content` resolve to carry speed alongside the def id). Acceptance: deadline ≈
      `hops × 12 / TIC_HZ` + slack; no premature "trip deadline passed" during a normal trip.
- [x] webgl: MoverLayer speculation rate from the bundle's `thing_speed()` by kind (replacing
      the codec `ticsPerTile(kind)` import — that call feeds a KIND into a DEF-shaped stub and
      dies with it). Acceptance: wolf glides at 0.5 tiles/s in the browser; `[mover] spec
      landed` errors stay small.

## P3 · Kill the snap-tween-snap (intent reliability)

- [x] Baseline measurement BEFORE fixing: soak `rd-npc` ≥ 10 trips with the browser open;
      count from `[mover]` console lines — trips vs armed specs vs two-snap trips vs stale
      arms. Acceptance: numbers recorded in `completed.md` (this is the evidence the fixes are
      judged against).
- [x] (added during execution — I4/I5, F6) `TicEstimate` tracks the OBSERVED wall↔tic rate:
      windowed re-anchor on fresh arrivals + learned rate (clamped around `TIC_HZ`) + a
      poison guard against serially-ahead stale-replay tics; `TicAnchor` carries the rate and
      hosts extrapolate with it. Acceptance: unit tests for drift/poison; live arms show `d`
      near the true elapsed (single digits, NOT climbing with page age).
- [x] MoverLayer: never DROP a live intent — buffer intents for unseen movers and arm on the
      pawn's first `State`; hold intents that arrive before the tic clock anchors and
      re-evaluate on `tick()` instead of returning on `d === null`. Stale/dedup guards
      unchanged. Acceptance: page-load-then-immediate-trip and spawn-then-move both glide.
- [x] event retention (first-pawns I2, server fix): sweep settled `event` rows older than the
      GC horizon on the event shard (ride the master's existing `gc`/`settle` cadence — same
      pattern as chat retention). Acceptance: a fresh zone subscribe delivers no ancient
      intents (spacetime sql shows the table bounded; browser console shows no stale-replay
      guards firing on re-subscribe).
- [x] I3 re-measure under soak (≥ 10 trips, with the edge's `on_applied` event replay from
      sim-self-heal + retention in place): every trip arms EXACTLY one spec — no absent, no
      doubled. If flakiness persists, root-cause at the edge (per-zone sub callback semantics)
      and fix there; record findings in `issues.md` either way.

## P4 · Authoritative snap + wrap

- [x] Confirm the authoritative-override story end-to-end: every `State` row snaps (landing
      clears the spec, interim resolve reseeds it, errors logged — first-pawns F8 path kept);
      record the tween-blend as a held knob in `ACTIONS.md` §Movement next to the
      re-anchor-every-N knob (F4 — future intent preserved, not built). Acceptance: docs-check
      green; console shows snap + error log on landing.
- [x] End-to-end acceptance in the browser (`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`):
      ≥ 10 consecutive trips ALL glide at 2 s/tile with no teleports other than authoritative
      snaps; screenshot + console evidence in `completed.md`.
- [x] Wrap: memory updated (speed model + snap fix), work-index row → done.
