# Forks — direction consistency

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — This is a PIPELINE stream, not a training stream {#f1}
_2026-08-08 · resolved at plan time · **the LoRA is frozen at `r20g07`**_

**Chosen.** `rd_diremph_anima_r20_g07` is an input, not a subject. No training run is planned, and
a consistency failure is not grounds to open one.

**Why.** The predecessor spent ~25 GPU-hours over eleven runs to reach a checkpoint that renders a
usable three-view set. The drift measured here is **24.9 luminance points between views of one
animal** produced by a pipeline that renders those views in three separate sampler calls with a
content-discarding anchor ([I2](issues.md#i2)/[I3](issues.md#i3)). That is a compositing problem
with a mechanism we can point at, and reaching for another training run would be answering it with
the most expensive tool available and the least evidence.

**Rejected — train a direction-consistency LoRA.** There is nothing to train it on: the corpus's
three views per species are *already* consistent (spread ≤ 9.0), so the model has seen the right
thing 701 times. The information is not missing from training; it is being discarded at inference.

**Rejected — go back and pick a different generation.** g07 and g08 are both retained
([checkpoints.md](checkpoints.md)) and either can be swapped in as a one-line change, but choosing
between them before the ruler exists ([P0](todo.md)) would be picking by eye on the exact axis the
eye is worst at.

## F2 — Cheapest lever first: prompt form, then anchoring, then batching {#f2}
_2026-08-08 · resolved at plan time · **ordered by cost, not by novelty**_

**Chosen.** [P1](todo.md) (prompt form) → [P2](todo.md) (anchoring) → [P3](todo.md) (batched pass).

The temptation is to open with the batched sampler pass, because it is the capability the new
hardware unlocks and it is the most interesting. It is also a graph rewrite. Meanwhile the prompt
form is **one constant** and already has evidence behind it — the prose form's saturation runs
48/20/28 across three views of one animal against the tag form's 4.4/6.4/4.1
([I1](issues.md#i1)). Shipping the interesting change first would fold that free correction into a
big diff and make the attribution unreadable.

**The risk, accepted:** P1 and P2 may close the spread enough that P3 never gets a fair trial, and
the 24 GB capability stays untested. Recorded so that outcome is a decision rather than a drift —
if P1/P2 clear the ceiling, P3 still runs once as a measurement.

## F3 — `cn` stays at 0.5; consistency is not bought by weakening control {#f3}
_2026-08-08 · resolved at plan time · **a hard floor, not a default**_

**Chosen.** `cn 0.5 / cn-end 0.9` is frozen for the stream. Any change that needs `cn` below 0.5
is out of scope and belongs to a different stream.

**Why it needs saying.** Lowering `cn` *would* reduce the measured drift, by letting the model
repaint each view freely toward whatever it thinks a wolf looks like — and it would take the
silhouette with it. [beat-e07 P4](../2026-08-02-lora-beat-e07/todo.md) already bought that lesson:
at `cn 0.2` the east sprite came out **a featureless white slab with a head stuck on**. A metric
that improves while the sprite gets worse is the failure mode this project has hit four times, and
`cn` is the single knob most able to produce it here.

**Consequence, stated so it is not mistaken for an oversight:** silhouette consistency is inherited
from the authored templates and is **not evidence about this stream's changes**
([I4](issues.md#i4)). Only interior metrics count.

## F4 — Luminance spread is the primary metric; saturation is secondary but not ignorable {#f4}
_2026-08-08 · resolved at plan time_

**Chosen.** Cross-direction **luminance spread**, against the corpus ceiling of **9.0**, is the
number every change is judged by. Saturation and absolute value are reported alongside and neither
may regress.

**Why luminance leads.** The game re-tints through layer maps, and only **brightness** survives
downstream — settled in the predecessor and unchanged. A hue that drifts between views is largely
cosmetic in the shipped asset; a *value* that drifts is visible in the game.

**Why saturation is still reported.** The layer-map decomposition finds material regions by colour
clustering. A fully desaturated sprite is not merely dull, it is **harder to decompose** — and ours
already measures 4.1–6.4 against 10.6–17.5 for a grey corpus wolf. [P4](todo.md) checks the
decomposition explicitly rather than assuming it survives.

**Why absolute value is tracked separately from spread.** Three views that agree with each other at
150 and disagree with the corpus at 87 would score perfectly on the primary metric while being
uniformly wrong. Consistency and correctness are different properties and the plan measures both.
