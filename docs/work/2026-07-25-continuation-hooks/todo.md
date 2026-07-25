# Continuation hooks — todo

_Phases, in execution order. Items move to [`completed.md`](completed.md) as they land + verify.
Ordering principle: **kill the inference layer first**, then make failure visible, then make the
handover useful — each phase makes the next one measurable._

This file is itself written to the P0 contract (every item is a `- [ ]` checkbox) — the detector it
describes reads this file.

---

## P0 — The checkbox contract (retire the inference layer)

- [ ] **Define the item rule**: an ITEM is a line matching `^\s*[-*]\s+\[[ xX]\]` in `todo.md` /
      `remaining.md`. Every other bullet is prose/detail and is NOT counted. Document in
      `docs/CONVENTIONS.md` (§ `docs/work/`). Acceptance: the rule is stated in CONVENTIONS and
      `work_check._open_items` implements exactly it, with no DONE-header heuristic left.
- [ ] **Normalize the corpus** — convert the 14 non-checkbox streams' `todo.md`/`remaining.md` items to
      checkboxes (`[x]` under DONE sections, `[ ]` elsewhere). Conservative: top-level bullets only;
      nested detail bullets stay prose. Acceptance: `rd work-check --doctor` reports 0 streams with
      "no items parsed" among non-done streams.
- [ ] **`docs-check` invariant `work-items`** — a non-DONE `##` section in a `todo.md` that contains
      bullets but no checkbox item is an ERROR (it is invisible work). A section with neither is fine
      (prose intro). Acceptance: deliberately un-checkboxing an item fails `docs-check`; tree green after.
- [ ] **Retire the heuristics** in `work_check.py`: `DONE_HEADER` scanning for item state, the
      `[:width]` half-sentence truncation path, and the bullet-based open-count. Acceptance: phantom
      open items across all streams = 0, measured by the corpus harness.
- [ ] **Progress signal from box counts** — the guard's progress signature uses `(open, done)` box
      counts per stream, so "ticked a box" is unambiguous progress. Keep git/worktree as a secondary
      signal for code-only turns. Acceptance: ticking one box resets the nudge counter; a no-op turn
      does not.

## P1 — Make failure visible (the silent-death class)

- [ ] **Decision log** — append every Stop verdict to `.git/rd-work/decisions.jsonl`:
      `{ts, session, stream, verdict, why, nudge, open, done}`. Acceptance: a nudge, a release, and an
      allow each write exactly one line; the file survives across sessions.
- [ ] **`rd work-check --doctor`** — per-stream table of what the parser sees (status, items open/done,
      blockers, stop-reason, binding) plus a `PARSE?` column flagging a non-done stream with zero items.
      Acceptance: run on the tree, output matches a hand-check of 3 sampled streams.
- [ ] **Wire doctor into `docs-check` as a WARNING** so an unreadable stream announces itself in the
      normal gate instead of vanishing. Warning-only — it must never block a commit.
      Acceptance: `docs-check` reports the warning for a deliberately-broken stream, still exits 0.

## P2 — Mechanism fixes (cheap, specific, independent of the above)

- [ ] **Execution-intent gate** — only nudge if the session actually wrote/edited a file this turn.
      The Stop hook fires on *every* turn end, so a mid-stream question currently gets the asker
      pushed back into stream work. Track per-session last-write in the bind hook; clear it on nudge.
      Acceptance: a turn with only `Read` calls never nudges; a turn with an `Edit` does.
- [ ] **`rd work focus <stream>` / `--clear`** — explicit session→stream override, beating the
      touch-based binding (which drifts: a session that logs to stream A then works B's code stays on
      A). Acceptance: `focus` wins over a later contradicting file touch until cleared.
- [ ] **Escalate on stall** — when the guard releases after `MAX_NUDGES` no-progress stops, auto-write
      `.stop-reason` recording the stall, so the next session sees it instead of re-nudging blind.
      Acceptance: the 4th no-progress stop writes the file; the following stop is silent because of it.

## P3 — The resume brief (replace the nudge with context)

- [ ] **`rd work brief <stream>`** — assemble the compact resume payload: current phase, next 3
      unchecked items **in full**, open blockers/forks, recent deviations, last 2 completed entries,
      and the design/intent docs to read. Acceptance: brief for `2026-07-25-primitive-graph` is under
      ~4KB and contains its next real action (vs 45KB across 6 files to reconstruct by hand).
- [ ] **Hook emits the brief** instead of the bare item list, keeping the four legitimate exits.
      Acceptance: a nudge shows the brief; length stays bounded (cap + "…N more").

## P4 — Planning assist (the gap hooks can't reach)

- [ ] **Granularity warning** in `docs-check` (advisory): flag an item over ~250 chars, or with ≥2
      sentences and no acceptance clause, as "not sized to execute". Warning-only, never blocking —
      it is a nudge to decompose, and the corpus has 67 such items today. Acceptance: count reported;
      no ERRORs introduced.
- [ ] **`/plan-phase` skill** (`.claude/skills/plan-phase/`) — reads a stream's `design`/`intent`/
      `current` + the named phase, and emits a checklist of one-action items, each with an acceptance
      criterion, for the user to ratify before it replaces the paragraph. Acceptance: run it on
      `2026-07-25-primitive-graph` P3 and produce a ratifiable checklist.
- [ ] **Decompose one real phase with it** end-to-end as the proof, and record what the decomposition
      changed about the items. Acceptance: the phase's items each name one action + a check.

## P5 — Verify the whole system

- [ ] **Corpus harness** — `bin/lib/work_check_selftest.py`: asserts the classifier against every
      stream (items, blockers, verdicts) so the next dialect drift fails loudly. This is the standing
      answer to "all eight bugs were invisible from the code". Acceptance: harness passes; breaking a
      classifier rule makes it fail.
- [ ] **Full hook matrix re-run** after every phase lands: blocked→allow, open work→block, nudge
      1..N→block, N+1→release + `.stop-reason`, progress→reset, read-only turn→allow, escapes→allow.
- [ ] **Update `docs-authority`** README/memory to point at this stream as the owner of the
      continuation half, so the two don't drift.
