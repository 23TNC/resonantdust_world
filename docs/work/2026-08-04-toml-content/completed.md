# Completed — TOML content

## 2026-08-04 · P0 — the truth frozen (2/2)

**The inventory** ([I1](issues.md#i1)): comment-stripped grep confirms FOUR features in
corpus code — and narrower than planned: zero `or`/`not` (earlier sightings were prose in
comments), so biome rules are pure conjunctions. Consumers pinned by file:line (wasm 84,
edge worldgen 62–82 + `/content`, worker 97, npc 210, `bin/dsl`, `subframe.py`); client
core, gateway, and the sim trio confirmed codec-only. The 17 `@on_destroy` stubs are dead
(zero `tile_has_hook` call sites). Also located the load-bearing rand: `Store::rand`
(vm.rs:263, SplitMix64 over `seed ^ salt·φ`) — the rule classifier must inherit it
verbatim or every scatter re-rolls.

**The oracle** (F5): `tests/golden.rs` dumps every registry (id order = line number),
every flat table exactly as consumers fetch them, per-thing visual parts, a 9,261-cell
worldgen sweep (t/h/e every 0.05, per-cell seeds — every band edge and scatter path), and
the four needs probes into `tests/golden/corpus.txt` (297 KB, committed). Two tests: the
byte-compare against the fixture (first divergent line named on failure) and a
determinism self-check. Blessed, then verified green without the env var. Regeneration is
DELIBERATE only (`BLESS_GOLDEN=1`).
