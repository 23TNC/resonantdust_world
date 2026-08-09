# Improvement audit — the exercise: identify possible areas of improvement — 2026-08-08

**What** (user, 2026-08-08): an exercise to identify possible areas of
improvement. Not a fix stream — a SWEEP: gather every improvement candidate
the project has accumulated (open issues, named successors, deferred
postures, drift, code debt, perf headroom), verify which are still real,
rank them, and hand back ONE document the next planning conversations can
be built from.

**Why now**: two perf streams just closed with "healthy, ceiling above the
tested range" — a natural pause point. Meanwhile candidates are scattered
across a dozen work folders' issues.md, the memory index, docs-check
warnings, and code comments. Nobody has ever collected them in one place
with statuses; planning keeps re-discovering them one at a time.

**Design stance**:
- The deliverable is a RANKED INVENTORY — [`findings.md`](findings.md) in
  this folder ([F1](forks.md#f1)): per candidate, the evidence, a verified
  STATUS, an impact class, a coarse cost, and the recommended successor
  shape. NOTHING gets fixed in this stream ([I3](issues.md#i3)).
- The sources are ENUMERATED up front ([F2](forks.md#f2)) — six lanes, so
  "did we look everywhere" has an answer: open work-stream issues +
  named successors; docs-check warnings + doc drift; the perf streams'
  open questions; code-level sweeps; architecture gaps named in
  design/intent/memory; and session-observed DX friction.
- Every candidate gets a verification bar ([F3](forks.md#f3)):
  VERIFIED-OPEN / STALE (dropped with a line saying why) / UNKNOWN (probed
  only when the probe is cheap and read-only).
- The ranking rubric is fixed before ranking ([F4](forks.md#f4)):
  player-visible correctness > consistency/data risk > performance ceilings
  > velocity & DX > polish; cost S/M/L.

**Exit**: findings.md standing alone — ranked, evidenced, statused — with
deliberate dev-postures excluded rather than re-litigated
([I1](issues.md#i1)); docs+memory truth pass; **the user's eyes close the
stream** and pick what becomes the next work.
