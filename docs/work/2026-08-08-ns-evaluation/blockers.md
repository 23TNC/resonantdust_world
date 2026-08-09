# Blockers — north/south evaluation

_What genuinely needs the user. A decision I could make belongs in [`forks.md`](forks.md); filing
one here is a manufactured pause._

## B1 — north/south need fresh labels at 768, and only the user can label them {#b1}
_2026-08-08 · raised at P0.4/P0.5 · **generation is done; the judging is not mine to do**_

**What blocks.** [P1](todo.md) cannot be finished and [P3](todo.md) cannot start. Every north/south
conclusion in this stream rests on **19 south and 13 north images**, and both sets were generated at
**512** — the resolution [F5](forks.md#f5) has since replaced with 768 because 512 was producing the
splayed-mask faces that make up most of the south rejects.

So the labels describe a pipeline we no longer run, and the sample is too small to separate a real
discriminator from a coincidence either way.

**Why it needs you.** Labelling is the ground truth. I can generate the sprites — and have: a fresh
batch at 768 is in `.staging/eval_data/new/`, covering e/s/n per seed through the real pipeline. What
I cannot do is decide which ones are acceptable, and the whole point of this stream is that no metric
we own reproduces that judgement on north or south.

**What would unblock it.** Sorting the new batch with `art eval-sort`, to roughly **n≥40 per
direction**:

```bash
art eval-sort                                   # shows the next unsorted image + the commands
art eval-sort <file> good
art eval-sort <file> bad anatomy
art eval-sort <file> borderline lineart --note "..."
```

The reason code matters more than the count ([F3](forks.md#f3)): `background` is deterministic and
already solved, `anatomy` is what the vision gate is being tuned against, and `borderline` stops a
judge being scored wrong for sharing your hesitation.

**Options, with a recommendation.**

1. **Sort the fresh 768 batch to n≥40 per direction.** *Recommended.* It is the only path that makes
   P1 and P3 answerable, and it also re-measures whether south is still 26%-acceptable now that the
   resolution defect is gone — which may be the more valuable number.
2. **Sort a smaller batch, n≈25 per direction.** Faster, and enough to see whether the gate's south
   number moves off 58% at all; not enough to trust a threshold.
3. **Accept east-only automation now.** The gate scores 94–97% on east against an 84% baseline and is
   usable today. North and south stay a human decision, and the stream's deliverable becomes a fast
   sorting loop rather than a judge. This is a legitimate outcome, not a failure
   ([P5](todo.md) says so).

**My read:** option 1 if the next training run matters, option 3 if it is soon. The east gate is
worth wiring in either way — it is the only automated check this project has that beats guessing.
