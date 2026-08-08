# Blockers — the east pipeline

_What genuinely needs the user. A decision I could make belongs in [`forks.md`](forks.md);
filing one here is a manufactured pause._

## B1 — P3.2: which method ships as the east default
_2026-08-08 · everything measurable is measured; this is a judgement about what we are optimising_

The plan says *"Have the user pick the method, or state none is good enough."* All the evidence is
in and the remaining choice is genuinely theirs, because the two candidates optimise different
things.

| method | trained `iou_ref` | untrained | cost |
|---|---|---|---|
| **S1** — the species' own corpus silhouette | **0.852** | **impossible** — needs a sprite that does not exist | 1 generation |
| **S2a** — the family representative's silhouette | 0.829 (0.832 on genuine generalisation cells) | works, with [I7](issues.md#i7)'s limit | 1 generation |
| S2a + best-of-3, gate-selected | ~0.87 est. | same | 3 generations |

**My recommendation: `S2a` (family) as the default, `S1` as an override when the species is in the
corpus.** S1 is better where it applies and cannot apply where the product is; S2a costs −0.023
overall and −0.067 on the cells that actually test generalisation, and it is the only one of the two
that works for a new animal. Making the *general* path the default and the *exact* path an opt-in
matches which one the pipeline will spend its life doing.

**What is NOT settled and should not be read into the numbers:**

- `iou_ref` is undefined for untrained species, so S2a's real-world quality rests on
  [I7](issues.md#i7) and the eye, not on 0.829.
- **Species whose identity is shape** (anteater, armadillo, warthog) do not work under either
  method. If those matter soon, the answer is [F7](forks.md#f7)'s edit-model stream, not a knob here.
- Best-of-3 is a real +0.040 for 3× the GPU; whether that trade is worth it is a throughput call.

**What I need:** a pick — S1-default, S2a-default, or "neither is good enough". [P3.3](todo.md)
(wiring it into `bin/art generate`) is one commit once the pick is made, and [P5](todo.md) closes
the stream after it.
