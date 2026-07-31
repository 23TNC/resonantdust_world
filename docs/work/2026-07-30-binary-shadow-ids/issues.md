# Issues — binary occlusion + stored caster ids

## I1 — `max` is the reason the refine was expensive, and the reason it can be cheap

Worth stating once, plainly, because it is the whole stream in a sentence.

The shadow value is `cov = max(cov, cc)` over every caster the corridor visits. **`max` keeps HOW MUCH
and discards WHICH.** So the deleted F7 refine had no way to sharpen an edge except to re-run the entire
search — DDA corridor, per-tile bucket fetches, `casterCover` across 8 caster slots — purely to
rediscover an identity the coarse pass had already computed and thrown away.

Measured then: **9.29 ms of a 10.88 ms pass, 85 %, for ONE light.**

That cost was never the evaluation. It was the search. Storing the winner deletes the search and leaves
one `casterCover` — which the plane-intersection work has since made ~3.5× cheaper.

**The 85 % figure is not today's number** and should not be quoted as one. It was taken at
`FINE_RATIO = 8` (64 fine texels per coarse); today it is 4 (16 per coarse), a 4× smaller multiplier, on
a gather that is 3.2–3.8× faster per caster test. Anyone re-deciding this should re-measure rather than
inherit the number.

## I2 — `any` restores order-independence, which unblocks the differential

`DIFFERENTIAL_WIRED = false` in `shadowGather.ts`. The lighting draw is `blend: "none"` — dirty rects are
**fully recomputed**, nothing is subtracted. The whole point of the differential is that a light's old
contribution can be reproduced **exactly** and negated, which `LIGHT_QUANT`'s integer quantisation exists
to guarantee.

`max` over an order-dependent caster set was never a problem for that on its own — but an **incumbent
early-out** would have been, because "whichever occluder we happened to find first" is not reproducible.

Binary occlusion removes the objection entirely: with `any`, the stored value does not depend on which
caster was found or in what order. **The early-out and the differential stop being in tension.**

Also already paid for: `coldShadowPrevRT` and `hotShadowPrevRT` are allocated unconditionally in the
resize, but the snapshot blit that fills them sits inside `if (DIFFERENTIAL_WIRED && shadowPrevRT)`. They
are **never written and never read — 4 MiB of ping-pong buffers idling**, waiting for exactly this.

Deliberately out of scope ([F5](forks.md#f5)) so the headline measurement attributes cleanly. Recorded
here so a successor does not re-derive it.

## I3 — Stale "u9" comments; the real encoding is `u7 | u1`

Several comments in `shadowGather.ts` describe the shadow slot as `u9` coverage. It cannot be: 16 slots ×
9 bits = 144 bits, and the texel is a 128-bit `uvec4`. The authoritative line is the `PRES_SLOTS` doc:

> each a `u7 coverage | u1 on-billboard` byte in the 128-bit shadow-cold texel (slot i at channel `i>>2`,
> bits `(i&3)·8` — 16 × 8 = 128 exactly)

confirmed by the read in `LIGHT_FRAG`: `((... >> shK) & 0xFFu) >> 1u ... / 127.0` — shift off the flag
bit, normalise by 127. So coverage today is **128 levels**, not 512.

Both I and the user reasoned from the `u9` comments during design. P1 rewrites this encoding anyway;
whatever replaces it must be documented in ONE place, and the stale duplicates deleted rather than left
to mislead the next reader. The same file also carries `// lights 0–6` / `// lights 7–13` on the presence
fetches, which describe a v2 layout that `tileSlot`'s own header says v3 retired.
