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
