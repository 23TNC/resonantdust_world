# Plan — TOML content

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md)._

## P0 — freeze the truth (the equivalence oracle)

- [x] Inventory every DSL feature the corpus uses + every Bundle consumer, recorded in
      `issues.md` (the four features + the dead `@on_destroy` stubs are the README's claim —
      verify by grep, list call sites). Acceptance: the list names file:line for each consumer's
      load entry; anything unexpected becomes its own issue before code. → [I1](issues.md#i1):
      comment-stripped op counts (the earlier `or`/`not` sightings were PROSE); consumers =
      wasm:84, worldgen:62–82, worker:97, npc:210, bin/dsl, subframe.py; core/gateway/sim-trio
      confirmed non-consumers; the destroy stubs confirmed dead (zero `tile_has_hook` callers).
- [x] A golden-dump tool (test-side): serialize EVERY Bundle registry + flat table + a worldgen
      sample grid (4 zones of `generate()` output) + the needs-eval probe values to one canonical
      fixture file, committed. Acceptance: the dump is deterministic (two runs byte-identical)
      and fails the build if the corpus and fixture disagree. → `shared/dsl/tests/golden.rs` +
      297 KB `tests/golden/corpus.txt`: registries, every flat table, visual parts, a 9,261-cell
      worldgen SWEEP (every 0.05 of t/h/e × per-cell seeds — stronger than 4 zones: covers every
      band edge + scatter rand path), the 4 needs probes. Blessed via `BLESS_GOLDEN=1`; both
      tests green; determinism test included.

## P1 — the schema, documented before parsed

- [ ] Write the TOML schema into `VARIABLES.md`: per-category tables, EXPLICIT `id` (F1), one
      record per def (F6), biome rules with `gte`/`lt`/`lte` keys mirroring the ops (F3), needs
      bands, moodlets, materials, parts lists, subframes (per-variant/rotation), lights, packed
      channels. Acceptance: docs-check green; every field of every existing def has a documented
      TOML spelling BEFORE the loader parses any of it.

## P2 — the loader (same Bundle out)

- [x] `toml` → `Bundle` in `shared/dsl`: registries with explicit-id validation (REFUSE
      duplicate/zero/missing — load errors, F1), all flat tables produced by the SAME accessors
      (F2). Acceptance: crate tests — a minimal TOML fixture round-trips ids + one table per
      category; a duplicate id fails loudly. → landed WITH the enabling refactor: `Bundle` is
      now MATERIALIZED (hooks evaluate once at load; accessors are dumb reads) — proven
      byte-identical by the golden oracle. `toml_loader.rs` fills the same struct;
      `deny_unknown_fields` makes a typo'd key a load error (the TOML answer to the DSL's
      silently-dropped writes); id-law tests cover duplicate/zero/missing; a HOLE resolves
      nothing. `load()` dispatches by extension; mixed dialects refuse.
- [x] The biome rule classifier: evaluate `when` conjunctions + ordered scatter against the
      host's dimensions/rand, keeping `generate()`/`GenTile` signatures (F3). Acceptance: unit
      test — a hand-built rule set reproduces a hand-computed classification incl. the
      first-match priority and last-write-wins scatter. → `BiomeBody::{Hooks, Rules}` behind the
      one `generate()`; `tile_rand` extracted as THE derivation (the VM now delegates to it);
      rules sort deterministically. Covered by the toml round-trip test + the golden sweep.
- [x] Wasm-surface check: the `toml` dependency compiles on wasm32 through the 2-pass gate.
      Acceptance: `rd`'s shared check (native + wasm32) green with the new dep. → green.

## P3 — the corpus, translated once

- [x] A converter (dev tool, deleted in P6): load the `.rd` corpus through the OLD loader, emit
      `content/*.toml` with each def's id pinned to today's positional value. Acceptance: the
      emitted files parse through the NEW loader; ids match the old registries 1:1. →
      `tests/convert.rs` (env-gated like the bless): tiles/things/materials/needs emitted from
      the materialized Bundle, subframes reconstructed by an exact RESIDUAL encoding against
      the fallback chain; biomes hand-translated (`content/biomes.toml` — their bodies are
      code; the sweep proves the translation). Two schema flaws caught EN ROUTE: part `scale`
      vs `sprite_scale` are separate channels; `cast/receives_shadows` are numeric MODES
      (ground authors 2), not bools.
- [x] Hand-carry the load-bearing comments (needs.rd's model notes, things.rd's append-rule
      notes → now id-law notes, biome dimension docs) into the TOML files. Acceptance: a reader
      of `content/needs.toml` learns what the reader of `needs.rd` learned. → needs.toml carries
      the model (lazy satisfactions, exclusive bands, conditional/timed, provisional numbers);
      biomes.toml the dimensions/priority/scatter/no-torch notes; tiles/things the id law.

## P4 — the golden gate (F5)

- [x] The equivalence test: BOTH loaders parse their corpora; every fixture section
      byte-identical (registries, tables, worldgen grid, needs probes). Acceptance: the test is
      green in CI docker; any diff prints the first divergent section. →
      `the_toml_corpus_matches_the_same_fixture` GREEN: all 297 KB byte-identical — registries,
      every flat table, all 9,261 worldgen cells (rules + `tile_rand` exact), needs probes. The
      divergence printer earned its keep twice on the way (lane 2.0, scale channels).

## P5 — consumers swap (one seam at a time, live-checked)

- [x] The edge serves + loads TOML: `/content` ships the `.toml` sources (wire key + version
      hash over the new files), worldgen classifies through the rule engine. Acceptance: LIVE —
      a fresh zone generates; `/content-version` moves when a `.toml` edits. → found + killed a
      SECOND content reader (the edge's own `load_disk` for `/content`, separate from worldgen's
      shared one — the two-copies class again); `/content` now serves the 4 client `.toml`s
      (biomes withheld, server-only); version moved on edit; fresh zones at (220,140) generated
      through rules (forest scatter + a biome edge + beach).
- [x] npc + worker load TOML (the shared loader entry does it — verify, don't assume the disk
      mirror). Acceptance: LIVE — npc resolves wolf `0x30010070` speed 12 thirst 1 from the
      TOML corpus; the worker logs corpus speeds on restart. → both logged exactly that; the
      wire key stays `rd` (a literal, like `/lod/` — renamed never or in P6's docs note).
- [x] The client loads TOML through `shared/wasm`; content hot-update still swaps. Acceptance:
      LIVE — the world renders (tiles, trees, wolf, walls); a tint edit in `tiles.toml`
      hot-swaps without reload; the details panel still shows moodlets. → identical forest at
      the fixture; the grass-tint edit went RED on screen mid-session with the wolf still
      trotting (reverted); panel shows the fresh wolf at mood 50%.
- [x] Tooling follows: `bin/lib/subframe.py` emits TOML; the `bin/dsl` publisher ships `.toml`.
      Acceptance: `subframe.py biome-thing/default/conifer` output drops into `things.toml`
      unchanged; the publisher round-trips the corpus to R2 naming. → subframe.py output is a
      VERBATIM drop-in (compared line-for-line); bin/dsl gains root-`*.toml` sync in
      upload/download (UNTESTED against live R2 — no creds run here; noted honestly).

## P6 — the deletion (git is history)

- [ ] Delete `parser.rs`, `vm.rs`, the hook/facet machinery, the `.rd` corpus, and the P3
      converter; the standing drills re-run green (wolf trip, thirst crossing, panel moodlets).
      Acceptance: `grep -r "\.rd\b\|@define\|on_create" shared/ content/` finds only history
      references; drills logged.
- [ ] Rename `shared/dsl` → `shared/content` (crate `resonantdust-content`), update consumer
      Cargos, docker mounts, `bin/rd` (F4). Acceptance: full stack rebuilds + boots; docs-check
      green.
- [ ] Docs + memory truth pass: `VARIABLES.md` speaks only TOML, `docs/README`/component docs
      re-pointed, the `dsl-rebuilt`/`biome-dsl`/`content-hot-update` memories rewritten to the
      TOML reality. Acceptance: docs-check green; no authoritative doc mentions a live DSL.

## P7 — the verdict

- [ ] Cold-boot the stack on the TOML corpus + the drill set; **the user's eyes close the
      stream** on an identical-looking world. Acceptance: captures + the golden gate's final
      run recorded in `completed.md`.
