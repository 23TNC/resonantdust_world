# Forks — docs migration

Decisions made during the migration + why. Chronological.

- **2026-07-14** · **What to do with superseded pre-rewrite designs** (`simulation*`, `gaps`,
  `data-shards`, `object-shard`). Options: **(a)** fold into the relevant component's `current/`;
  **(b)** delete; **(c)** a `docs/archive/` tree. **Chose (c).** **Why:** they aren't the
  design/intent/current of any *live* component (folding pollutes a component with dead framing),
  but they have historical value (delete loses it). `docs/archive/` with a "superseded by" map
  keeps history out of the way. Extended `CONVENTIONS.md` with the archive rule. Reversible.
- **2026-07-14** · **dev docs vs assets.** Migrated the dev *doc* files (`art-style`,
  `texture-paths`, `texture-layout/`, `sprite-gen-plan`) into `components/dev/{textures,
  scripts/art}`. **Left** `docs/prompts/*` in place — they're sprite-gen *template assets*, not
  docs, so they don't belong under `docs/components/`. **Deferred** the `bin/` → `dev/scripts/`
  reorg — that's a *code* move, out of scope for a docs pass. Redirect if you'd shape dev
  differently (e.g. a single `dev/art` vs split `textures`/`scripts/art`).
