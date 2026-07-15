# Completed — docs migration to the components/work convention

Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md) across all components. Done work,
chronological.

- **2026-07-14** · **Convention + trees established** — `docs/CONVENTIONS.md` (the how);
  `docs/components/README.md` (the component map); `docs/intent/` staging (+ pathfinding);
  `docs/work/spacetime-rewrite/` (completed/remaining/todo/issues/forks/blockers).
- **2026-07-14** · **`shard` component migrated** — `docs/spacetime-tables/` and
  `docs/spacetime-implementation/` → `docs/components/server/spacetime/modules/shard/`:
  design (`tables`, `event-dsl`, design README), intent (`events`, `lifecycle`, `hot-cold`,
  `risks`, `event-reference`), current (`README` + `divergences`), plan (`stages/` S0–S7 +
  `cleanup` + plan README) + component README. 100 relative links rewritten to the new depth; old
  folders removed. Dead modules kept OUT of design, placed in current (reality) + plan (deletion).
- **2026-07-14** · **codec / gateway / pipeline / pixijs migrated + sync staged** — codec:
  `docs/references/*` + `object-model*.md` → `components/shared/codec/{design,current}` (flags
  the object-model open decisions → blocker B-1). gateway: `gateway.md` → intent. pipeline:
  `pipeline-generalization.md` (design) + `world-on-pipeline.md` (intent). pixijs: `lighting.md`,
  `de-lighting.md` (design) + `client.md`, `zones-to-screen.md` (intent). Cross-cutting
  `sync.md` → `docs/intent/`. 33 outgoing + 12 incoming link-sets rewritten; `docs/references/`
  removed. Component READMEs written.
- **2026-07-14** · **legacy archived + dev docs migrated** — superseded pre-rewrite designs
  (`simulation`, `simulation-plan`, `gaps`, `data-shards`, `object-shard`) → `docs/archive/` with
  a superseded-by map (+ CONVENTIONS archive rule). Dev docs → `components/dev/textures/design`
  (`art-style`, `texture-paths`, `texture-layout/`) + `components/dev/scripts/art/plan`
  (`sprite-gen-plan`). Decisions logged in [`forks.md`](forks.md). All 270 doc links verified
  resolving.
- **2026-07-14** · **pathfinding reconciled** — found a pre-existing `docs/pathfinding/README.md`
  (real 0.2.3 design) I'd missed; dropped my stub, moved the real doc to `docs/intent/pathfinding/`,
  fixed the index description. All 277 doc links verified resolving.
- **2026-07-14** · **issues folder folded in** — `docs/issues/00X` detail (001–005 + the 006
  fork) folded into `work/spacetime-rewrite/issues.md` + `forks.md` (full problem→options→choice→
  why, links recomputed); `docs/issues/` removed. Matches the single-file convention.

- **2026-07-14** · **The stream's last real items.** `client/core`: `intent/client.md` was filed
  under `pixijs` but described the **headless Rust client** end to end (host API `src/api.rs`,
  login flow, config, the `headless` binary) — pixijs appears only as a future *consumer*. **No
  split needed**; it was simply misfiled. `git mv`'d to `components/client/core/intent/`, inbound
  links fixed (`docs/intent/sync.md`, pixijs README), `client/core/README.md` added.
  `docs/repo-layout.md`: **keep, scoped** — it's the physical tree + the 2026-07-11 rename key
  (needed to read pre-restructure history); the component map is the conceptual truth. Merging would
  bury a rename table in the map, deleting would lose the key; both now say so and cross-link.
  The header caveat ("`git mv` breaks code-comment doc-paths; fix when that code is next touched")
  is **void** — all 15 were repointed in `d81e1e7`. Verified: **0 broken relative links** across
  `docs/`.
  Still open by design: `server/edge` folders (lazy-create — edge hasn't been worked; the map entry
  is its home) and `docs/prompts/*` (moves *with* the `bin/` → `dev/scripts/` reorg, which is itself
  proposed-not-done — not worth moving twice).
