# Forks — plane intersection

_Decision points, options, which we chose and why._

## F1 — Delete `shadowCover`, or keep it as a cheap reject?

**SUPERSEDED by [F6](#f6) on the "keep it around" half — the quad is deleted outright, not retained
behind a switch. The cheap-reject question below still stands as a P2 measurement.**

- **(a) Delete it.** The inversion is the same predicate and already early-outs on `t` then `s`.
- **(b) Keep a cheapened version** — e.g. a bounding-circle or AABB reject before the inversion.

**Chosen: (a), with (b) reserved for measurement.** The inversion's own `t` range check IS a cheap
reject — it fails in a subtract, a divide and a compare, which is less than any bounding test that has to
be constructed first. (b) only earns its place if P2 measures the inversion as still miss-dominated, and
then it should be a *cheap* reject (squared distance to the caster anchor vs reach), never the quad.

## F2 — How to restore the reach bound `projectTop` was providing

**2026-07-27 — the justification below is RETRACTED (user challenge). Reach is already enforced
elsewhere, and the identity argument does not hold.**

- `buildPresence` gives each tile only the lights whose reach circle covers it, so a texel never
  considers a light that cannot reach it. Reach IS clamped, one level up.
- The corridor walks from the texel TOWARD the light, so any caster able to shadow that texel lies on
  the walked segment and is visited regardless of clamping.
- Corridor and brute both call the same `casterCover`, so a change to the projected quad's SHAPE applies
  to both equally. It cannot make them disagree.

I took the claim from `projectTop`'s own comment ("bounded by reach is also what preserves the corridor
identity") and repeated it into this fork without testing it — the same unverified-mechanism pattern as
[I6](issues.md#i6).

**What may actually depend on the clamp** is dirty bookkeeping, not identity: a moving light dirties its
REACH BOX, so a shadow extending past reach would leave a stale smear in tiles outside that box. That is
concrete and testable — move a light and look for a trail beyond its reach circle.

**And the divergence may be an improvement.** Truncating a shadow at an arbitrary circle is a fudge; a
shadow that ends where the light stops mattering is the physical behaviour. Part of P0's measured 26 %
may be the ray test being MORE correct, not less.

_Original framing, kept because the option analysis is still the right menu:_

`projectTop` clamps a projected corner to `reachU`, so a caster's shadow can never leave the light's
reach box.

- **(a) Distance test on P**: reject when `length(P − L.xy) > reachU`. One `dot` and a compare.
- **(b) Clamp `t`** so the intersection cannot occur beyond reach.
- **(c) Rely on the walk's reach box** and drop the per-caster bound.

**REOPENED.** (c) — rely on presence and drop the per-caster bound — is now the leading option, not the
wrong one: presence already bounds which lights a texel sees, and the corridor already walks the segment
that contains any relevant caster. Decide it in P1 by TESTING, not arguing: build the ray gate with no
reach bound, run corridor↔brute identity, then move a light and check for a stale trail outside its reach
circle. If identity holds and there is no smear, (c) is correct and the bound is dead weight. If a smear
appears, (a) — a squared-distance reject on `P`, no `sqrt` — is the cheap fix.

## F3 — Where `SHADOW_BASE_PUSH` goes

It currently offsets `Yb` in the quad corners, pushing the shadow base south into the caster's footprint
to close a seam between sprite and shadow.

- **(a) Fold into `Ac.y`** before the inversion.
- **(b) Widen the accepted `t` range** slightly below 0.
- **(c) Drop it** and see whether the seam is still visible now the silhouette is sampled per-tap.

**Open — decide during P1.** (a) is the most faithful translation but shifts the whole card, which also
moves the tip; (b) affects only the base, which is what the fudge was for. (c) is worth one screenshot
before committing to carrying a fudge whose original cause may no longer exist — but the burden of proof
is on removing it, not keeping it.

## F4 — Two rays, or keep a variable tap count?

- **(a) Exactly two** (light ± radius) plus the centre for the straddle case, as proposed.
- **(b) Keep the graduated ladder** from [moving-lights](../2026-07-26-moving-lights/completed.md).

**Leaning (a), but it must beat (b) on evidence.** (b) is measured: the ladder is 1.46× over fixed-16,
the binary variant was rejected because capping taps bands any penumbra wider than the cap, and the odd
tiers anchor the umbra. (a) proposes to get a *smooth* result from 2 samples by filling analytically
rather than averaging — which, if F5 lands, is strictly better than any tap count. If F5 does not land,
(a) degrades to the 2-tap tier that was already measured as the worst of the ladder, so this fork is
downstream of F5 and must not be decided before it.

## F5 — Where the wedge gradient comes from

Two rays return near-binary silhouette opacity, so "one hit, one miss" yields 1/2 rather than a ramp.
Filling the wedge needs a continuous quantity.

- **(a) Analytic from `s`/`t`.** Exact where the boundary is the card's own edge, since that edge is a
  known line. Free. But our shadow's outline is the *silhouette* inside the card, not the card edge, so
  this covers the wedge's outer envelope and not interior detail.
- **(b) Signed distance field in the atlas.** One fetch gives distance-to-edge; coverage is a
  `smoothstep` over the penumbra width. **O(1) in penumbra width** — it deletes the reason the ladder
  exists rather than tuning it, and costs nothing to store because coverage is recoverable from an SDF.
  Costs: `bin/art` must generate it; the co-pack layout is definition-authoritative so it is a corpus
  re-bake; 8 bits must span ~25 texels of penumbra; and an SDF models the NEAREST edge, so a conifer tip
  — a comb of branches inside one penumbra width — is exactly where it will disagree with ground truth.
- **(c) Binary-search the edge** between the two rays: 2 + log2(N) samples for N-tap quality.

**Open.** (a) is nearly free and should be done regardless as the envelope case. (b) is the real answer
and the only option whose cost does not grow with softness, but it is the only one that touches the art
pipeline — prototype it on ONE sprite and A/B against 16 taps before committing to a re-bake, because
the branch-comb case is where it is most likely to disappoint. (c) is the fallback if (b) disappoints.

## F6 — Replace-and-delete, not A/B behind switches {#f6}

**2026-07-27 — RESOLVED by the user, and it supersedes F1 and the stream's original acceptance rule.**

> _"We are not going to tack a ton of extra stuff to try and enable/disable it. We are going to replace
> and delete the current shadow implementation as we go."_

- **(a) Replace and delete in the same change.** Each phase writes the new predicate and removes what it
  replaced, in one commit. **CHOSEN.**
- **(b) Keep both behind a uniform** (`uRayTest`) and A/B them, deleting the loser later.

(b) is what the first attempt did and it earned its reversal. Three costs, all paid on 2026-07-27:

1. **It doubled the surface without doubling the confidence.** `uRayTest`, `uPredDiff` (four modes),
   `__raytest`, `__preddiff`, `__shadowfp` — every one needed wiring, a uniform, a dial, and a reload to
   exercise, and none of it shipped. Two of the instrument's own modes were silently dead for a full
   measurement round because they sat inside the wrong guard.
2. **A switch defaults one way, so the other path is never really exercised.** `rayTest = 0` meant the new
   code compiled and never ran outside a deliberate probe. Scaffolding that is off by default is
   scaffolding that rots.
3. **It invites keeping the old thing "just in case",** which is exactly how a codebase ends up with two
   implementations of one predicate — the very thing this stream exists to remove. Deleting as we go is
   the property that makes the change real rather than additive.

**Consequence — the bit-identity gate is withdrawn.** The old plan required P0–P2 to change no pixels.
That only makes sense when the old path survives as a reference. Replacing outright means P0's measured
~26 % divergence is the intended outcome, and the gate becomes: corridor↔brute identity holds, it looks
right at three zooms, and it is faster. `git` is the reference; `checkpoint/pre-plane-intersection` is the
tag to diff against.

**What this does NOT license.** Dropping a behaviour because it is inconvenient, without checking what
depended on it. That cuts both ways — see [F2](#f2), where I asserted the reach clamp was a load-bearing
invariant and it turned out presence already enforced reach. Test what an old fudge was doing before
either keeping or deleting it.
