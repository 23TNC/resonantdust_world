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
