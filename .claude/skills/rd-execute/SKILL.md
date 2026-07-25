---
name: rd-execute
description: Execute a docs/work/ stream to completion — arms the continuation hook so the session keeps going through phase boundaries instead of stopping after each task. Use when the user says go / run / execute / continue a work stream, or names a work folder to work through. Do NOT use for planning (that's rd-plan) or for answering questions about a stream.
---

# rd-execute — run a work stream

Arms the continuation hook for **this session** and executes the named stream's plan until it is
complete, blocked, or stalled.

The hook is **off by default** so normal conversation is never interrupted. Arming is the user
saying *go*. Nothing else turns it on.

## 1. Resolve the stream

If the user named one, use it. Otherwise:

```bash
bin/rd work doctor
```

Pick the stream with open items that matches what they asked for. If two plausibly match, ask —
arming the wrong stream wastes a whole session. If the chosen stream has **0 open items**, say so
and stop; there is nothing to execute.

## 2. Arm and read the brief

```bash
bin/rd work arm <stream>
```

This prints the brief: current phase, the next open items in full, open blockers, what just landed.
**That brief is your working context** — it exists so you don't re-derive the stream from its whole
folder. Read `README.md` / `forks.md` / `issues.md` only for the decisions your specific item touches.

Before writing code, re-read the authoritative docs for what you're about to change —
`docs/components/<c>/design/` and `intent/` outrank both the plan and the existing code
(`docs/README.md` has the precedence order). A plan item that contradicts `design/` is a plan bug:
log it in `issues.md` and raise it rather than building it.

## 3. Execute

Work the open items **in order**, one at a time. For each:

- Do the work. Follow the repo's conventions; build toward the documented design, never simplify it
  away for the task in hand.
- Verify it against the item's acceptance criterion. An item without one is under-planned — decide
  the check yourself, state it, and record it.
- **Tick the box** `- [x]` in `todo.md` and append a dated entry to `completed.md` saying what landed
  and *how it was verified*. Items stay where they are — the box is the move, not a file transfer.
- Commit when a coherent unit is done (standing permission; no push, no PR).

Keep going across phase boundaries. Do not stop to ask "shall I continue?" — that pause is the thing
this skill exists to remove. If you hit a fork (a decision you can resolve), resolve it, log it in
`forks.md` with the reasoning, and continue.

## 4. Stop only on a real exit — and record it first

| Exit | What to write |
|---|---|
| **Complete** | every box ticked + `completed.md` entries; the hook auto-disarms |
| **Blocked** | a row in `blockers.md` — what blocks, why it needs *them*, options, your recommendation |
| **Plan error** | the defect in `issues.md` (or `deviations.md` if the code already departed), then raise it |
| **Other** | one line in `.stop-reason` |

Each of these auto-disarms the hook. If you stop without recording one, the hook will push you to
continue — that is working as designed, not a malfunction. After three no-progress nudges it writes
`.stop-reason` itself, disarms, and hands back.

To stop early on the user's instruction: `bin/rd work disarm`.

## 5. Report

Summarise what landed, what's left, and anything you recorded (blockers, forks, deviations). Be
honest about what was verified versus assumed — say plainly if something is untested.
