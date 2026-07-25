---
name: rd-plan
description: Plan a docs/work/ stream — open a new one, or decompose a phase into executable checkbox items each with an acceptance criterion. Use when the user wants to plan, scope, break down, or re-plan work, or when a phase's items are too coarse to execute. Never arms the continuation hook; planning is a conversation. Use rd-execute to then run the plan.
---

# rd-plan — turn intent into an executable plan

Produces the thing `rd-execute` consumes: a phase whose items can each be *done*, not interpreted.

**This skill never arms the hook.** Planning is a conversation and must be interruptible.

## Why this exists

Measured across the corpus: median plan item **142 characters**, dozens over 250. Items describe
phases, not actions. That is a direct cause of premature stopping — a vague next step forces the
session to re-plan before it can act, and planning invites ratification, which is a stop. The fix is
upstream, in the plan.

## Mode A — decompose an existing phase

1. Read, in precedence order: `docs/components/<c>/design/` → `intent/` → `current/` → the stream's
   `README.md`, `forks.md`, `issues.md`. **`design/` wins outright** — re-read it rather than
   trusting the ticket. Verify anything a `current/` file claims before planning on it; those caches
   lag, and a stale row becomes a phantom project.
2. Restate the phase's goal in one sentence, and its **done condition**.
3. Decompose into items that each satisfy:
   - **one action** — one verb, one target. If it needs "and" or a semicolon to describe, split it.
   - **an acceptance criterion** — the observable check. Name the command, the debug overlay, the
     identity diff, the console value. "Works correctly" is not a criterion.
   - **ordered** — an item may depend only on items above it.
   - **under ~250 chars.** `rd docs-check` warns past that.
4. Write them as checkboxes under a `## P<n> — <name>` heading in `todo.md`:
   ```
   - [ ] <one action>. Acceptance: <observable check>.
   ```
5. **Present the decomposition for ratification before it replaces the prose.** Show the old
   paragraph and the proposed items side by side. The user owns the plan.

## Mode B — open a new stream

Create `docs/work/YYYY-MM-DD-<slug>/` per [`docs/CONVENTIONS.md`](../../../docs/CONVENTIONS.md):
`README.md` (what + why + design stance), `todo.md` (phases as checkbox items), `completed.md`,
`issues.md`, `forks.md`, `blockers.md`, `deviations.md`. Add a row to `docs/work/README.md` — the
index is enforced by `docs-check`, so an unlinked folder fails the tree.

Carry the **full future intent** into the plan. Never trim the documented design down to what the
current task needs — the plan is where that intent survives session limits and re-planning.

## Always

- `bin/rd docs-check` before finishing. Keep the tree green.
- Record decisions as you make them: `forks.md` for a choice you resolved (with the rejected options
  and why), `issues.md` for a problem hit, `blockers.md` only for what genuinely needs the user.
- A **fork is yours to resolve**; a **blocker needs them**. Don't file a decision you could make as a
  blocker — that is a manufactured pause.

## Check your work

```bash
bin/rd docs-check          # invariants, incl. oversized-item warnings
bin/rd work brief <stream> # what a resuming session will actually see
```

If the brief doesn't tell a fresh session what to do next, the plan isn't finished.
