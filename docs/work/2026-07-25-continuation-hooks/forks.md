# Continuation hooks — forks

_Decision points, options, which we chose and why. Convention:
[`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

## F1 — Where does item state live? · resolved 2026-07-25

**Checkboxes in the markdown**, not a generated sidecar or an issue-tracker schema. See
[`issues.md` I1](issues.md#i1) for the options. The deciding argument: any *second* store of item state
will drift from the prose, and we already have a documented instance of that failure class
(`current/` lagging code → the phantom-project trap). One marker the human writes anyway is the only
option with no drift surface.

## F2 — New stream vs. more `docs-authority` phases · resolved 2026-07-25

Split into this stream. `docs-authority` owns the *audit* (`docs-check`, the front door, the invariants)
and is largely delivered; the continuation half is substantial, multi-phase, and has a different
failure model (fails open and silent, vs. the audit which fails loud). Keeping them together would
bury this plan under a mostly-done stream. Cross-linked both ways so they can't drift.

## F3 — Does the granularity check block? · resolved 2026-07-25

**Warning only.** 67 items in the corpus would trip it today, so making it an ERROR would either
block the tree on day one or force a mass rewrite of plans that are otherwise fine. Its job is to
prompt decomposition at the moment a phase is picked up, which a warning does.

## F4 — Autonomy: does the hook push through *phase* boundaries? · resolved 2026-07-25

Yes — unchanged from [`docs-authority` F6](../docs-authority/forks.md) (dial = default,
blocking-but-bounded, retuned to `WORK_CHECK_MAX_NUDGES=3`). This stream does **not** turn the dial up
further; it makes the existing setting actually work. The escapes (blocker row, `.stop-reason`,
`SKIP_WORK_CHECK=1`) stay untouched, and P2 *narrows* firing (execution-intent gate) rather than
widening it.

## F5 — What happens to unchecked boxes under a DONE header? · resolved 2026-07-25

The contract says *the checkbox is the single source of truth for item state*, which meant dropping
`work_check`'s DONE-header heuristic. Measuring first showed that would newly count **15** items —
so the corpus was audited rather than the rule bent:

- **7 were lying boxes.** `mrt-bakes` B1–B5 and `caster-lut` C5a say "DONE (moved to completed.md)" in
  the header; the work is in `completed.md` and the boxes were simply never ticked. **Ticked.**
- **8 are honestly open.** `2026-07-24-lightmap-resolution` "(Optional, F6)" / "(Open, later)" and
  `webgl-engine` W4f/W4h deferred sub-items are real deferred work parked under a delivered phase.
  **Left unchecked** — a deferred item *is* open work, and whether to nudge on it is the job of the
  stream's status / `.stop-reason`, not of a regex over its heading.

This is the contract earning its keep on day one: the ambiguity was always there, and the heuristic
was hiding it in both directions.

## F6 — Arm the hook explicitly (`/rd-execute`) instead of inferring intent · resolved 2026-07-25

- **2026-07-25 · user:** "we likely want a `/rd-execute` skill to run a work folder and arm the hook.
  This way we can handle conversations normally most of the time until I say go."
- **Adopted, and it replaces P2's execution-intent gate.** That gate was a heuristic — "did the session
  edit a file this turn?" — guessing at intent from side effects, which is the *same mistake* as
  inferring item state from prose. Arming is the explicit version: the hook is **disarmed by default**,
  `/rd-execute <stream>` arms it for that stream, and it disarms on completion, on a recorded blocker,
  or on `/rd-execute --stop`.
- **Why this is strictly better:** it removes the entire false-positive class (a question asked
  mid-stream can never drag the asker back into stream work), it makes the binding explicit instead of
  drift-prone (F2/P2's `rd work focus` folds into it), and it puts the autonomy dial where it belongs —
  the user says "go", rather than the hook deciding how much autonomy to take.
- **The through-line:** every fix in this stream replaces an *inference* with a *declaration* —
  prose→checkbox (F1), touch-heuristic→arming (F6), guessed-intent→explicit go. That is the design
  principle, and anything left inferring is a future bug.

## F7 — Consolidate the work-folder files? · RECOMMENDATION, needs the user's call

- **2026-07-25 · user:** "we likely want to consolidate files, you seem to prefer one file and crossing
  stuff out as opposed to individual files like todo and complete."
- **The observation is fair and I should own it:** `todo.md` files already mark phases `✅ DONE` *in
  place* with a pointer to `completed.md` (8 of 35 do this) rather than actually moving items out — so
  the documented `todo → completed` move is half-practised at best. Pretending otherwise is the drift.
- **Measured before recommending** (35 streams, 188 files): `forks.md` 154KB/35 files, `issues.md`
  108KB/30, `completed.md` 124KB/27 — all three heavily used and large. `blockers.md` 18 files, **10 of
  them near-empty** ("None open."); `deviations.md` exists in only 8. So the ceremony is concentrated in
  the *sparse* files, not the big ones.
- **Recommendation — two separable changes:**
  1. **Items never move; `[x]` IS the move.** (Adopting now — it needs no file restructure and falls
     straight out of [F1](#f1).) `todo.md` keeps every item for the life of the stream; `completed.md`
     stops being a second copy of the list and becomes purely the **dated verification log** — which is
     what its 124KB already is in practice. This is exactly the "cross it out" ergonomic, but with a
     marker a machine can read.
  2. **Fold the sparse files into the plan** — `blockers.md` + `deviations.md` become `## Blockers` /
     `## Deviations` sections of the plan file (the detector reads a section instead of a file; no
     capability lost). **Held for the user's yes** — it rewrites ~26 files across 35 streams and edits
     `CONVENTIONS.md`, which is a large, hard-to-reverse change to *their* system, and they asked what I
     think rather than telling me to do it.
- **Rejected: `~~strikethrough~~`.** It retains dead content in the authoritative file, which is the
  precise failure `delete-dont-deprecate` exists to prevent, and `docs-check`'s decay-marker invariant
  already bans `~~` in `design/**` + the top authorities. `- [x]` is the same ergonomic without the
  noise, and it is machine-readable. **Keep `forks.md` + `issues.md` separate** — 262KB combined would
  bury the reasoning the stream exists to preserve.
