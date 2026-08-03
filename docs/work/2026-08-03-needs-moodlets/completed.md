# Completed — needs & moodlets

## 2026-08-03 · P2 — the shard remembers (3/3, re-shaped by F7)

The planned "new NEED table" was a plan bug ([F7](forks.md#f7)): the pawn PAYLOAD sidecar is
the documented home ("parts, and later inventory/stats/needs/…") and its pipe — zone-keyed
rows, claim-slaved writes, edge fan, client decode — already runs for PART. Landed instead:

- **codec**: `NEED` (op 2, `need_id:8|sat:8|set_tic:16`) + `MOODLET` (op 3,
  `moodlet_id:8|0:8|grant_tic:16`; expiry DERIVED from corpus duration) opcodes with
  `upsert_need`/`upsert_moodlet` (rewrite-in-place by id) + decoders; 4 new codec tests
  (in-place upsert, re-grant refresh, lane packing, PART coexistence).
- **verbs**: `SET_NEED` (10: obj write, need_id, sat 0..=255) and `GRANT_MOODLET` (11)
  in `action.rs` + `ACTIONS.md`; client-open at the edge door (like MOVE_TO — no ownership
  model yet, noted in the allowlist).
- **pawn module**: `set_need`/`grant_moodlet` reducers — the MODULE composes the splice
  (read current payload → upsert → log + projection, one transaction, idempotent by uid), so
  the worker's NEEDS arm just relays and its read set grows by nothing.
- **bindings** regenerated for BOTH consumers (edge `src/bindings/pawn` + `st-bindings`);
  worker/orchestrator/master rebuilt + restarted; module republished via `rd redeploy --run`.

**Verified LIVE**: browser `queue([10, wolf, 1, 25])` + `queue([11, wolf, 3])` → SQL on
`resonantdust-dev-pawn-0` shows the wolf's payload = `NEED(1, 25, tic 1616)` +
`MOODLET(3, tic 1616)`, zone 98 — the whole chain (edge door → grouping → claim → worker
relay → module splice) in one pass. HONEST GAP: the client-side fan is mechanism-proven
(the same opcode-agnostic frame PART uses) but not yet OBSERVED client-side — the core
drops non-PART opcodes until P5's decode; P5's acceptance closes it.

## 2026-08-03 · P1 — the corpus speaks needs and moodlets (5/5)

`shared/dsl` gains the `<need>` + `<moodlet>` registries (material-style: direct `@define`,
1-based append-stable ids), `NeedParams { label, deplete, bands }` with up to 4
`NeedBand { moodlet, lo, hi }` slots, `MoodletParams { label, mood, duration }`
(duration 0 = conditional, F2), and `&thing.needs.<0..7>` → `thing_needs()` /
`thing_needs_table()` (stride 8, 0 = empty — the `thingLayout` shape). Indexed slots use
bare digits (the `packed.<i>` precedent; the I8 hazard needs a scalar sibling to bite,
documented as "don't add one").

Corpus: `content/data/needs.rd` authors thirst (deplete 21600 tics = 1 h wall at 6 Hz;
bands Thirsty [0.10, 0.35) −0.15, Dehydrated [0, 0.10) −0.40, EXCLUSIVE) + timed
Quenched (+0.20, 3600 tics) so the timed form parses from day one; the wolf's
`:data @define` carries `"thirst &thing.needs.0 set`. `VARIABLES.md` gains the
"Needs & moodlets" section.

**Verified**: 3 new loader tests (registry round-trip with two bands coexisting on one
need; unauthored need → name-label/0-deplete/no-bands; wolf resolves thirst + typo slot
drops + table stride) and the REAL-corpus smoke test extended (thirst registered, drains,
2 bands all naming registered moodlets, wolf carries it) — 50 tests green in the
spacetime docker image. (Note: the docker `test` service trips on a stale
host-glibc codec test binary; `cargo test -p resonantdust-dsl` is the clean lane.)
