# Completed — TOML content

## 2026-08-04 · P7 — the cold boot (17/17 boxes; the user's eyes close the stream)

Edge + master/orchestrator/worker + npc bounced together on the TOML-only tree: the npc
re-resolved `def 0x30010070 speed 12 thirst 1` and re-adopted its wolf; the client
cold-loaded the IDENTICAL forest at the fixture view; the golden gate's final run is
3/3 green (the TOML corpus still byte-matches the frozen `.rd` fixture). What remains is
the user's look — and the ongoing invariant is now the fixture itself: a deliberate
content tune re-blesses with `BLESS_GOLDEN=1`; an accidental table drift fails the build.

## 2026-08-04 · P6 — the deletion + the rename (3/3)

parser.rs, vm.rs, the hook/facet machinery, the `.rd` corpus, the converter, the `.rd`
fallback walk AND the client's boot embed — all deleted (git holds them). The embed bit
back: the webgl client imports its offline-fallback corpus via `?raw` at BUILD time, so
deleting the `.rd` files black-screened the boot until the embed became the four client
TOMLs. `load()` refuses non-TOML sources by name. The needs_eval fixture + the wasm crate
test were translated. Live exception, deliberate: `content/visual/manifest/*.rd` are
bin/art's ART manifests — same extension, never corpus.

The rename (F4): `shared/dsl` → `shared/content`, crate `resonantdust-content` (the wasm
re-export keeps its `dsl` alias for the TS surface). En route the deletion flushed out a
THIRD private copy of the content walk — the worker's `load_speeds` — which broke the
moment the dialect moved and is now the one shared `read_content_dir`. Full stack rebuilt
+ redeployed: worker `kinds=11`, npc `def 0x30010070 speed 12 thirst 1`, `/content` = 4
TOMLs, the world renders, trips flowed throughout.

Docs + memory: VARIABLES.md speaks TOML in every authored-spelling block; shared/AGENTS;
bin/dsl syncs root `*.toml` + `visual/` (manifests); memories `dsl-rebuilt`/`biome-dsl`
deleted, `toml-content` written, two descriptions updated. The golden fixture STAYS as
the table-drift guard (a deliberate content tune re-blesses via `BLESS_GOLDEN=1`).

## 2026-08-04 · P5 — every consumer on TOML, live (4/4)

The swap concentrated in TWO seams: `read_content_dir` (shared — TOML-wins-else-`.rd`;
serves worldgen + worker + the golden/convert tests, which now walk their `.rd` files
explicitly) and the edge's own `load_disk` for `/content` — a SECOND content reader
found live (the two-copies class F2 exists to kill; it had silently kept `/content` on
`.rd` through the first restart). Everything downstream — npc, wasm client — flowed
through `load()`'s dialect dispatch untouched.

**Live evidence**: `/content` lists the 4 client `.toml`s (biomes withheld,
server-only) and `/content-version` moves on a `.toml` edit; the worker logged
`corpus speeds loaded kinds=11` and the npc `def 0x30010070 speed 12 thirst_need 1` —
the SAME packed def off explicit ids (F1's whole point); the client renders the
identical forest, the grass tint went RED on screen mid-session and reverted (content
hot-update on TOML, no reload, wolf trotting throughout); fresh zones at (220,140)
generated through the RULE classifier (forest scatter, a biome edge, beach sand);
`subframe.py` now emits verbatim drop-in TOML. Honest gap: `bin/dsl`'s new root-toml
R2 sync is code-reviewed only (no live R2 creds run here).

## 2026-08-04 · P3+P4 — the corpus translated, THE GATE GREEN (3/3)

The converter (`tests/convert.rs`, env-gated, throwaway) emits tiles/things/materials/
needs from the materialized Bundle with ids pinned to today's positions; subframes come
back out through an exact RESIDUAL encoding against the fallback chain (base ← the
never-authored `[15][15]` slot, per-axis rows, then per-slot corrections) — the emitted
minimal keys match what the corpus originally authored (v5 absent because it equals the
union default, exactly as `subframe.py` computed it). Biomes were translated BY HAND
(7 defs — their `.rd` bodies are code) into `content/biomes.toml` with the dimension/
priority/no-torch commentary carried over; needs.toml carries the whole needs model.

**The gate** (F5): `the_toml_corpus_matches_the_same_fixture` is GREEN — the TOML corpus
reproduces the `.rd` fixture BYTE-IDENTICALLY: every registry, every flat table, all
9,261 worldgen sweep cells (proving the hand-translated rules + the inherited
`tile_rand` draw exactly), the needs probes. The divergence printer caught two real
schema flaws en route: part `scale` and `sprite_scale` are SEPARATE channels (a pawn
slot's art scale vs the pre-atlas ingest pair), and `cast/receives_shadows` are numeric
MODE lanes (ground tiles author 2) that a boolean schema would have silently flattened.

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
