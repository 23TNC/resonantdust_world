# Plan — TOML content

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md)._

## P0 — freeze the truth (the equivalence oracle)

- [ ] Inventory every DSL feature the corpus uses + every Bundle consumer, recorded in
      `issues.md` (the four features + the dead `@on_destroy` stubs are the README's claim —
      verify by grep, list call sites). Acceptance: the list names file:line for each consumer's
      load entry; anything unexpected becomes its own issue before code.
- [ ] A golden-dump tool (test-side): serialize EVERY Bundle registry + flat table + a worldgen
      sample grid (4 zones of `generate()` output) + the needs-eval probe values to one canonical
      fixture file, committed. Acceptance: the dump is deterministic (two runs byte-identical)
      and fails the build if the corpus and fixture disagree.

## P1 — the schema, documented before parsed

- [ ] Write the TOML schema into `VARIABLES.md`: per-category tables, EXPLICIT `id` (F1), one
      record per def (F6), biome rules with `gte`/`lt`/`lte` keys mirroring the ops (F3), needs
      bands, moodlets, materials, parts lists, subframes (per-variant/rotation), lights, packed
      channels. Acceptance: docs-check green; every field of every existing def has a documented
      TOML spelling BEFORE the loader parses any of it.

## P2 — the loader (same Bundle out)

- [ ] `toml` → `Bundle` in `shared/dsl`: registries with explicit-id validation (REFUSE
      duplicate/zero/missing — load errors, F1), all flat tables produced by the SAME accessors
      (F2). Acceptance: crate tests — a minimal TOML fixture round-trips ids + one table per
      category; a duplicate id fails loudly.
- [ ] The biome rule classifier: evaluate `when` conjunctions + ordered scatter against the
      host's dimensions/rand, keeping `generate()`/`GenTile` signatures (F3). Acceptance: unit
      test — a hand-built rule set reproduces a hand-computed classification incl. the
      first-match priority and last-write-wins scatter.
- [ ] Wasm-surface check: the `toml` dependency compiles on wasm32 through the 2-pass gate.
      Acceptance: `rd`'s shared check (native + wasm32) green with the new dep.

## P3 — the corpus, translated once

- [ ] A converter (dev tool, deleted in P6): load the `.rd` corpus through the OLD loader, emit
      `content/*.toml` with each def's id pinned to today's positional value. Acceptance: the
      emitted files parse through the NEW loader; ids match the old registries 1:1.
- [ ] Hand-carry the load-bearing comments (needs.rd's model notes, things.rd's append-rule
      notes → now id-law notes, biome dimension docs) into the TOML files. Acceptance: a reader
      of `content/needs.toml` learns what the reader of `needs.rd` learned.

## P4 — the golden gate (F5)

- [ ] The equivalence test: BOTH loaders parse their corpora; every fixture section
      byte-identical (registries, tables, worldgen grid, needs probes). Acceptance: the test is
      green in CI docker; any diff prints the first divergent section.

## P5 — consumers swap (one seam at a time, live-checked)

- [ ] The edge serves + loads TOML: `/content` ships the `.toml` sources (wire key + version
      hash over the new files), worldgen classifies through the rule engine. Acceptance: LIVE —
      a fresh zone generates; `/content-version` moves when a `.toml` edits.
- [ ] npc + worker load TOML (the shared loader entry does it — verify, don't assume the disk
      mirror). Acceptance: LIVE — npc resolves wolf `0x30010070` speed 12 thirst 1 from the
      TOML corpus; the worker logs corpus speeds on restart.
- [ ] The client loads TOML through `shared/wasm`; content hot-update still swaps. Acceptance:
      LIVE — the world renders (tiles, trees, wolf, walls); a tint edit in `tiles.toml`
      hot-swaps without reload; the details panel still shows moodlets.
- [ ] Tooling follows: `bin/lib/subframe.py` emits TOML; the `bin/dsl` publisher ships `.toml`.
      Acceptance: `subframe.py biome-thing/default/conifer` output drops into `things.toml`
      unchanged; the publisher round-trips the corpus to R2 naming.

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
