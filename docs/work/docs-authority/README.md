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

- **Stop hook** (Claude Code) — `.claude/settings.json` runs
  [`bin/hooks/stop-check.sh`](../../../bin/hooks/stop-check.sh) at the end of every turn, in two stages:
  - **Stage 1 · docs-check** — docs/ must be compliant. Broken → **exit 2**, failures to stderr, turn
    can't end. Loop guard: an identical failure set twice → release (a non-convergent case can't trap
    the turn). Escape: `SKIP_DOCS_CHECK=1`.
  - **Stage 2 · work-check** (`--enforce`) — if docs are clean, a *silent premature pause* (open,
    unblocked, executable work in the active stream + no `.stop-reason`) also blocks, pushing me to
    continue. Bounded: progress guard (no new `completed.md` entry since the last nudge → release),
    recency window (`WORK_CHECK_WINDOW_MIN`, default 180, so a fresh/idle session doesn't nag), and
    escapes (a `blockers.md` row, a `.stop-reason` file, or `SKIP_WORK_CHECK=1`). Dial = default
    (blocking-but-bounded); see [`forks.md`](forks.md) F6.
- **git pre-commit** — [`bin/hooks/pre-commit`](../../../bin/hooks/pre-commit), symlinked to
  `.git/hooks/pre-commit`, runs the full `docs-check` and **refuses the commit** if docs/ is broken
  (work-check does *not* gate commits — WIP is fine to commit). Escape: `SKIP_DOCS_CHECK=1 git commit …`.

**Install (pre-commit is not tracked — a fresh clone must redo it):**
`ln -sf ../../bin/hooks/pre-commit .git/hooks/pre-commit`. The Stop hook is tracked (in
`.claude/settings.json`) and applies automatically. *(A future `rd docs install-hooks` could automate
the symlink — deferred; see todo P3.)*

## Files
- [`todo.md`](todo.md) — the phased plan (P0–P4).
- [`forks.md`](forks.md) — the load-bearing decisions (new-folder-vs-entry-point, enforcement,
  tombstone policy, sync.md fate).
- `completed.md` — created as items land.
