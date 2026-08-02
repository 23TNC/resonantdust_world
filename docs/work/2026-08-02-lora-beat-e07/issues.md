# Issues — beat e07

_Problems hit, and what the evidence actually showed. Findings recorded at plan time are marked as
such — they were read out of the prior streams' logs, not measured under this one._

## I1 — The jitter refutation stopped 5.4× short of `e07`'s own condition {#i1}
_2026-08-02 · read at plan time from [`sprite-eval-trust`](../2026-07-26-sprite-eval-trust/completed.md)_

Run-5 tested "does scale variance matter?" by jittering `fill`, and the answer was recorded as a
clean refutation — correctly, because it varied exactly one thing. But the range it covered is not
the range that separates the shipping model from the failures:

| dataset | fill sd |
|---|---|
| v1 — what `e07` was trained on | **0.1478** |
| v2 (P2 rebuild, pinned) — runs 3/4/5 | 0.0032 |
| v2 + jitter — run-5 | 0.027 |

Run-5 moved 0.003 → 0.027. `e07`'s data sat at **0.148**, still **5.4×** more variable than the
jittered set and **46×** more than the pinned one. So the finding that survives is "jitter of this
magnitude changes nothing", not "scale variance doesn't matter" — the latter was never tested at
`e07`'s magnitude.

This is not a criticism of that experiment, which did what it set out to do. It is the reason
scale normalisation is [F2](forks.md#f2)'s first target rather than a closed question.

## I2 — `prep_train` silently swaps the upscaler when ComfyUI is down {#i2}
_2026-08-02 · read at plan time from `bin/lib/prep_train.py`; **live right now**_

The prep "falls back to LANCZOS automatically if ComfyUI is unreachable, so the prep never
hard-fails on a box outage." That is a sensible default for a convenience script and a **trap for
a single-variable experiment**: the upscaler is one of the three P2 variables this stream is trying
to isolate, and a rebuild run while the box is down silently produces a *different dataset* under
the same command.

It is live: ComfyUI is currently **not running** on the unraid box, is **absent from the autostart
list**, and its restart policy is `no`. So the very next dataset rebuild would take the fallback
path by default.

[P0](todo.md) makes the fallback loud (opt-in via an explicit flag) before any rebuild happens.
The same class of bug already cost this project once — P2's scale normalisation was a **silent
no-op for the whole corpus** and looked plausible throughout, found only by measuring the achieved
fraction.

## I3 — P2 bundled three changes, so runs 3/4/5 are not attributable {#i3}
_2026-08-02 · read at plan time from [`sprite-gen-quality`](../2026-07-25-sprite-gen-quality/completed.md)_

The P2 rebuild changed, in one step:

1. **upscaler** — LANCZOS → ESRGAN (Remacri), outline sharpness 43.1 → 90.1
2. **scale normalisation** — unnormalised → longer side pinned to 0.85, fill sd 0.1478 → 0.0032
3. **resolution** — 768 → 1024²

Every model trained since inherits all three, and every model trained since has lost to `e07`.
No run has isolated any of them. Run-5's single-variable discipline is exactly the right tool and
was pointed at a fourth variable (jitter *within* 2) instead.

Each of the three was independently well-justified by measurement — this is not a claim that P2 was
wrong. It is that "measurably better inputs" and "a better-performing model" were assumed to follow
from one another, and three runs now say otherwise for at least one of them.

## I4 — The 2080 Ti's VRAM ceiling shaped every config decision on record {#i4}
_2026-08-02 · read at plan time_

Run-4 held **10,379 MiB of 11,264** — within 26 MiB of its predicted budget. So `dim 48 / alpha 24`
and `grad-accum 4` were not chosen as optima; they were the largest that fit. Any comparison
between those settings and something the 3090 can now hold is a comparison against a constraint,
not against a considered baseline.

Recorded so [P3](todo.md) does not read the old numbers as a tuned starting point — and so the new
headroom gets spent one turn at a time rather than all at once, which would recreate exactly the
attribution problem in [I3](#i3).
