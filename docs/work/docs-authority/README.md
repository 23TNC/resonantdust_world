# Work: docs-authority — make the docs a load-bearing external memory

_Work-stream README. Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Touches: the whole
`docs/` tree (meta) + **dev/scripts/rd** (the audit) + `.claude/` (the hook)._

## Why this exists

The user's ask was "restructure docs into a robust authoritative form I can rely on as external
memory." Investigation found the **structure is already sound** — the convention in
[`CONVENTIONS.md`](../../CONVENTIONS.md) is complete, and the authoritative surface already exists:

- **Cross-component authority:** [`VARIABLES.md`](../../VARIABLES.md) · [`TABLES.md`](../../TABLES.md)
  · [`ACTIONS.md`](../../ACTIONS.md) — already outrank the code.
- **Per-component authority:** `components/<c>/design/` + the [component map](../../components/README.md).
- Reasoning in `notes/`, flowing state in `work/`, future intent in `intent/`.

So the deliverable is **not a new "authoritative folder"** — that would just be a fourth place to
drift. The real defect is **maintenance discipline**: the exact failures the user named — "not
deleting, not updating, marking as deprecated" — and the convention *already forbids* all three
(delete-don't-deprecate; move-between-state-files; verify-before-you-work). Nothing **forced**
compliance. The tree held live violations left by the agent despite the rule (all cleared in P0 —
see [`completed.md`](completed.md)):

- `intent/sync.md` was tagged `⚠️ stale` and left in place (the anti-pattern).
- `~~strikethrough~~` "DONE" annotations in `work/*/todo.md` (banned by the row convention).
- `current/` caches that lag with no freshness signal — the precise cause of the 2026-07-14
  "phantom project" bug written up in [`CONVENTIONS.md`](../../CONVENTIONS.md) (lines 48–59).

## The thesis

**A discipline problem is not fixed by willpower — it's fixed by a forcing function.** This stream
delivers a machine-checkable audit (`bin/rd docs-check`) wired to an **automatic hook** (user's
choice, 2026-07-19) so the convention's own rules fail *loudly* the moment they're violated, plus
a single **front-door index** so "external memory" has one entry point instead of six.

Scope, in order of value: (1) bring the tree into compliance with the convention we already have;
(2) give it a front door; (3) build the audit; (4) wire the hook; (5) add freshness provenance.

## The hooks (how the forcing function fires)

Two hooks, both over `bin/rd docs-check`:

- **Stop hook** (Claude Code) — `.claude/settings.json` runs
  [`bin/hooks/stop-docs-check.sh`](../../../bin/hooks/stop-docs-check.sh) at the end of every turn.
  Green → silent, exit 0. Broken → **exit 2**, the failures print to stderr and the turn can't end
  until they're fixed. **Loop guard:** it hashes the failure set; if the *same* failures recur (agent
  tried, no change) it lets the turn end rather than trap the session — any progress re-blocks on the
  new set. Escape: `SKIP_DOCS_CHECK=1` in the hook env.
- **git pre-commit** — [`bin/hooks/pre-commit`](../../../bin/hooks/pre-commit), symlinked to
  `.git/hooks/pre-commit`, runs the full audit and **refuses the commit** if docs/ is broken. Escape:
  `SKIP_DOCS_CHECK=1 git commit …`.

**Install (pre-commit is not tracked — a fresh clone must redo it):**
`ln -sf ../../bin/hooks/pre-commit .git/hooks/pre-commit`. The Stop hook is tracked (in
`.claude/settings.json`) and applies automatically. *(A future `rd docs install-hooks` could automate
the symlink — deferred; see todo P3.)*

## Files
- [`todo.md`](todo.md) — the phased plan (P0–P4).
- [`forks.md`](forks.md) — the load-bearing decisions (new-folder-vs-entry-point, enforcement,
  tombstone policy, sync.md fate).
- `completed.md` — created as items land.
