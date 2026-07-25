# Forks — docs-authority

_Decision points: options · choice · why. Chronological append. Executes:
[`docs/CONVENTIONS.md`](../../CONVENTIONS.md)._

## F1 — A new authoritative *folder*, or an entry-point over the existing surface?

- **2026-07-19** · The ask was "an authoritative folder." **Options:** (a) mint a new
  `docs/authority/` and move the canonical shapes into it; (b) keep the existing distributed
  authority (`VARIABLES`/`TABLES`/`ACTIONS` + `components/*/design/`) and add a single front-door
  index over it.
  **Chosen: (b).** The authority already exists and already outranks the code; a new folder would be
  a *fourth* place for the same facts to drift, which is the disease, not the cure. What's missing
  is a single door and enforcement — hence `docs/README.md` (P1) + the audit (P2), not a new tree.

## F2 — Enforcement mechanism

- **2026-07-19** · **Options:** automatic hook (runs the audit every turn / commit) · script run
  on request · session-start checklist. **Chosen: automatic hook** (user decision, 2026-07-19).
  **Why:** it's the only option that doesn't ultimately depend on the agent's in-the-moment
  discipline — which is the exact thing that has failed repeatedly. Cost: some hook output per turn;
  mitigated by `--quiet` (surface only on failure).

## F3 — `☠ GONE` tombstones: delete outright, or keep bounded in index files?

- **2026-07-19** · delete-don't-deprecate says remove superseded content; but a *map/index* file's
  "X is gone because Y" line is genuine external memory — it answers "why isn't this here?", which
  is exactly what stops re-derivation. **Options:** (a) ban tombstones everywhere; (b) allow a
  single bounded "Retired (why absent)" list **only** in map/index READMEs, ban them in every
  content file.
  **Chosen: (b).** The audit's decay-token check (P2 invariant 1) therefore **exempts** index/map
  files (`**/README.md` at a tree root that functions as a map) and **enforces** deletion in
  `design/`, `intent/`, and the three top authorities. A tombstone that grows past a couple of lines
  or drifts into a content file is a violation.

## F4 — `intent/sync.md` — rewrite or delete?

- **2026-07-19** · The file's own note says "`valid_at` deleted 2026-07-15, model is tic-based now
  … rewrite against the tic or delete; do not build from it." The server half is superseded twice
  over; the client half (synced clock + render-delay `D` + interpolate) is *implemented* in
  `client/core/src/clock.rs`. **Options:** (a) rewrite the whole doc against the tic; (b) delete the
  doc, fold the surviving client-sync intent into `components/client/core/intent/`.
  **Chosen: (b).** The live part belongs to a component (client/core), not to cross-cutting staging;
  the dead part is a superseded design that git already holds. Rewriting a cross-cutting staging doc
  for a feature that's now single-component would re-create the drift. (P0 item.)

## F5 — Hook granularity: Stop vs PostToolUse vs commit-only

- **2026-07-19** · **Options:** PostToolUse-on-Write/Edit (fires on every doc edit — noisiest,
  earliest) · Stop (once per turn — catches the turn's net state) · pre-commit only (catches
  nothing mid-session but gates the commit). **Chosen: Stop + pre-commit** to start — Stop gives
  per-turn feedback without firing on every keystroke; pre-commit is the hard gate. Revisit toward
  PostToolUse only if turn-level feedback proves too late; toward commit-only if Stop proves noisy.
  Tunable once we see real signal.

## F6 — Continuation hook (`work-check`): firing condition + autonomy dial

- **2026-07-19** · User proposed a Stop hook that re-prompts me to **continue** when I paused with
  unfinished, unblocked work — automating [[execute-dont-relitigate]] + [[decide-and-proceed]].
  **The trap (rejected trigger):** "todo.md non-empty." A todo file's job is to hold open items;
  firing on that would forbid every legitimate checkpoint (including the turn that *built* this
  folder). **Chosen trigger:** silent premature pause only — active stream + executable-now items +
  no open blocker + no user-facing fork + **no recorded stop-reason**. Side effect (desired): forces
  me to *record why I stop*, which is the real behavioral gap.
  **Autonomy dial (user sets; default proposed):** this hook drives *open-ended* work (code,
  commits) with less human-in-loop than `docs-check`, so it needs a governor. **Default:** bounded
  by the progress guard (two no-progress stops → allow stop) + the records-reason escape hatch — so
  it never loops unsupervised. **Turn-up option:** push through phase boundaries autonomously.
  **Turn-down option:** pause at each phase boundary for review even when unblocked. Left as a dial
  because how much unsupervised execution is acceptable is the user's call, not mine.
- **2026-07-19 · Dial set → DEFAULT (blocking-but-bounded).** User: "wire with your default." So
  `work-check --enforce` is stage 2 of the Stop hook: it blocks a premature pause, but the progress
  guard + recency window + `.stop-reason`/blocker escapes bound it (never loops, never mis-fires on a
  fresh/idle session). Not turned up (no push through phase boundaries) and not left advisory. Retune
  by editing `bin/hooks/stop-check.sh` or setting `WORK_CHECK_WINDOW_MIN` / `SKIP_WORK_CHECK`.

- **2026-07-25 · F6 retune (dial unchanged, bounds loosened).** P6's audit showed the *default* was
  effectively "one nudge, then free" — the guard keyed on `completed.md` alone, which a multi-turn
  phase doesn't touch. The dial stays at **default (blocking-but-bounded)**, but the bound is now
  `WORK_CHECK_MAX_NUDGES` (default **3**) consecutive *no-progress* stops, where progress counts any
  stream-state edit, commit, or working-tree change. This is the turn-up option ("push through phase
  boundaries") in practice, without changing the dial's semantics: the escapes (blocker row,
  `.stop-reason`, `SKIP_WORK_CHECK=1`) are untouched, so it still never loops unsupervised. Turn it
  down again by setting `WORK_CHECK_MAX_NUDGES=1`.
