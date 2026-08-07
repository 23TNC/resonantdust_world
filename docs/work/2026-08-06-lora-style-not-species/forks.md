# Forks — style, not bestiary

_A choice I resolved, with what was rejected and why. A fork is mine; a [blocker](blockers.md) is
the user's._

## F1 — Caption for style and body plan; drop species tokens {#f1}
_2026-08-06 · resolved at plan time · **the stream's single variable**_

**Chosen.** Captions carry the convention tags, the body plan and the direction. No species name,
no family name. At generation the species comes from the PROMPT and is resolved by the base model's
own knowledge; the LoRA supplies only the rendering convention.

**Why.** [I1](issues.md#i1) — a species token backed by 3 images is memorisation competing against
animagine's internet-scale knowledge of the same animal, and it loses while consuming capacity.
Removing it leaves 701 examples teaching exactly one thing.

**Rejected — keep species tokens and add images.** Correct, and it is a data-acquisition project
of a different size. Worth doing; not doable this week, and this test costs one run.

**Rejected — keep species tokens but down-weight them.** There is no clean mechanism, and it
preserves the competition rather than removing it.

**Rejected — drop the body plan too.** Body plans have 27–498 examples each, which is a real
signal, and [I12 there](../2026-08-02-lora-beat-e07/issues.md#i12) showed the contrast between them
is what teaches quadruped = limbless horizontal mass. Keep it.

**The risk, stated plainly:** if the base's idea of a tiger is strong enough to override the
convention, style-only captions could produce well-drawn animals in the WRONG style — the exact
failure Illustrious showed at R14. [P3](todo.md)'s cat-versus-wolf comparison is what catches it.

### F1 addendum — the rule is a WHITELIST, not a species blacklist
_2026-08-06 · resolved at P1.1_

[P0.2](todo.md) found the vocabulary splits cleanly: a dozen phrases carry 233–234 examples each,
and the other 229 tokens carry ≤3. That makes the rule mechanical and safe to state positively.

**KEEP** — every `rd_*` tag (convention, body plan, direction) plus this exact phrase whitelist:

```
side profile · side view · facing right          (east)
front view · facing the viewer · facing forward  (south)
back view · facing away · seen from behind       (north)
single creature · full body · white background   (composition)
```

**DROP** — every other non-`rd_` token. By construction that is exactly the family and species
names, since nothing else survives the ≤3-examples cut.

**Why a whitelist rather than a species list.** A blacklist needs an exhaustive enumeration of 229
names and silently keeps anything it misses — and a missed species token is precisely the failure
this stream exists to remove. A whitelist fails the safe way: an unrecognised token is dropped, so
the worst case is a slightly thinner caption rather than a leaked 3-example concept.

**`rd_animal` is KEPT** (645/701) — it is a convention tag, not a species, and its absence on the
other 56 is itself signal.

## F2 — Direction emphasis AND the per-direction split, in that order {#f2}
_2026-08-07 · resolved at the user's two proposals · **run both; emphasis first**_

The user proposed two fixes for [I9](issues.md#i9) in quick succession: *"what if we separated this
and generated a south specific lora?"* and *"can we apply weight to tags? maybe we apply heavy
emphasis on north east south during the training"*. They are not alternatives — they attack the
same diagnosis at different costs, so both run.

**Chosen order — run-20 (emphasis, full corpus) first, then run-17 (south-only) and run-18
(east-only).** Emphasis is the cheaper hypothesis: it keeps all 701 images, so if it fixes south
the split is unnecessary and the stream's "one concept, 701 examples" thesis stays intact. The
split is the fallback that trades data for isolation.

**On tag weighting, stated so it is not asked again: kohya has NO per-token loss weighting.** The
`(tag:1.4)` syntax is an inference feature of the ComfyUI/A1111 CLIP encoders; the training text
encoder tokenizes it as literal characters. The three levers that do exist are caption order +
`keep_tokens`, token repetition, and per-subset dataset repeats. Run-20 uses the first two.

**Why run-18 (EAST-only) is in the queue and run-19 (north-only) is not.** A south-only LoRA alone
is unfalsifiable: if it is bad, that is either "north was not the problem" or "234 images is below
the floor", and the result cannot tell them apart. East at 701 is the one cell already known good,
so east-only is the control that separates them. North-only is written but unqueued — if the split
wins it follows trivially, and if it loses north-only was wasted GPU.

**Why the split is NOT a repeat of [I12](../2026-08-02-lora-beat-e07/issues.md#i12).** I12 recorded
that quad-only training made `rd_quadruped` a constant on 459/459 and therefore uninformative.
`rd_south` becomes a constant on 234/234, which looks identical and is not: I12 hurt because ONE
model still had to discriminate body plans at generation time from a corpus that no longer
contrasted them. Here direction is chosen by **selecting the LoRA**, so no model is ever asked to
discriminate it, and the body-plan tags still contrast within each subset.

**Steps are matched, not reduced.** The split datasets use folder repeats `3_animal`, so
234 × 3 = 702 images per epoch against run-16's 701, and 12 epochs is 2112 steps in every run.
Per-image exposure triples — that is the specialisation, and the overfit risk the per-generation
samples exist to catch.

**The cost of the split, recorded rather than discovered later:** 234 images instead of 701,
against a stream thesis that rests on 701 examples of one concept. The defence is that the concept
narrows too — the convention in one pose rather than across three — so it needs less data. Whether
234 clears the floor is precisely what run-18 measures.
