# Completed — TOML content

## 2026-08-04 · P2 — the loader (3/3), riding on the materialization refactor

**The enabling move**: `Bundle` stopped being a bag of parse trees with hook-running
accessors and became MATERIALIZED data — the `.rd` path now evaluates every hook exactly
once at `load()` (a `mod rd` that dies with the DSL) and every accessor is a dumb read.
The golden oracle proved the refactor changed NOTHING observable: byte-identical fixture,
all 50 crate tests green (two Node-API tests rewritten to their observable contracts;
the dead `tile_has_hook`/`run_hook`/Node accessors deleted with their dead test).

**The TOML side** (`toml_loader.rs`): serde schema mirroring VARIABLES.md with
`deny_unknown_fields` — a typo'd key REFUSES the load, the answer to the DSL's
silently-dropped writes; the id law (F1) enforced in `place()` (duplicate/zero/missing
refuse; a hole holds an empty slot that resolves nothing); parts arrays with the
subframe fallback chain reproduced PER COMPONENT (`v3.r1 → v3.e → v3 → r1 → e →
default`, tails into the flat `sprite_anchor`); biome `when`/`scatter` rules behind
`BiomeBody::Rules`, deterministically ordered. `tile_rand` extracted as THE scatter
derivation — the VM delegates to it, so `.rd` and rules draw identically. `load()`
dispatches by extension; mixed dialects refuse loudly.

**Verified**: 50 + 2 golden tests green in docker; the 2-pass gate (native + wasm32)
green with the `toml`/`serde` deps.

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
