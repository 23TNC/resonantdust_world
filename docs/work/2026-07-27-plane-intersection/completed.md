# Completed — plane intersection

_Verification log: what landed and HOW it was checked. Append-only; authoritative for what is done._

## 2026-07-27 · P0 instruments + the baseline

- **`__shadowfp(cls)`** — FNV-1a over the raw shadow-cold u32s plus summary stats (nonzero, sum,
  on-billboard count). Reproducible across reloads, so "bit-identical" stays falsifiable after the code
  it describes is deleted.
- **`__preddiff(n)`** — 0 off · 1 any disagreement · 2 quad says SHADOW / ray says LIT · 3 the reverse.
  `casterCover` returns 1.0 into shadow-cold where the two predicates differ, so the shadow map becomes a
  disagreement map.
- **BASELINE captured** (zoom 1, lean 1.0, 3 torches, 661 casters):

  | | hash | nonzero | sum | on-billboard |
  |---|---|---|---|---|
  | 16 forced taps | `1269869647` | 108 164 | 8 513 410 | 28 448 |
  | graduated ladder | `1111980347` | 107 920 | 8 523 562 | 28 448 |

## 2026-07-27 · P0 RESULT — the predicates DISAGREE. P1 cannot be a deletion.

**The stream's founding assumption is refuted as stated.** Measured twice on independent loads:

| | slots |
|---|---|
| shadowed | 164 487 |
| **disagreeing** | **43 631 — 26.5 %** (second run 27.2 %) |
| quad YES / ray NO | 21 157 |
| quad NO / ray YES | 27 565 |

It is **bidirectional and large**, not a thin systematic offset. A base-push-sized fudge would show as a
sliver on one side only; this does not.

(`2 + 3 ≠ 1` is expected, not an instrument fault: slots are MAX-accumulated over casters, so one slot can
hold caster A disagreeing one way and caster B the other — counted once in mode 1, once in each of 2 and 3.)

**Cause 1 CONFIRMED — the `reachU` clamp.** `projectTop` clamps a projected corner to the light's reach,
and runs the corner out to reach when `k <= 0`; the ray inversion has no concept of reach at all. Isolating
to ONE light and varying reach:

| reach (tiles) | shadowed | disagreeing | % |
|---|---|---|---|
| 8 | 15 426 | 3 336 | 21.6 |
| 20 | 54 278 | 10 335 | 19.0 |
| 48 | 55 391 | 6 598 | **11.9** |

More reach ⇒ less clamping ⇒ less disagreement, monotonically. That is the clamp.

**Cause 2 OPEN — a reach-independent floor of ~12 %.** It does not vanish as reach grows, so something
else differs. `SHADOW_BASE_PUSH` (0.75 units at the base only) is too small to explain 12 %. Candidates
not yet isolated: the `pos || neg` both-windings test in `shadowCover` degenerating on a self-intersecting
quad when the projection flips; and the `t`/`s` half-open range bounds vs the `>= 0 || <= 0` sign test.

### What this means for the plan

P1 as written — "delete `shadowCover`, the inversion already answers it" — **is not safe**. The two are not
one predicate computed twice; they encode materially different behaviour, and ~26 % of shadowed texels
would change. P0 existed precisely to catch this before the deletion, and it did.

The performance argument in [I1](issues.md#i1) is untouched: the quad build is still redundant *work* on
the miss path. But the replacement has to REPRODUCE the quad's behaviour, including reach, rather than
assume the inversion already does.

### Method note

The first disagreement sweep read each shadow-cold `u32` as a single value and reported 98 %. That is
wrong: the word packs **16 light slots as bytes across 4 lanes** (`b8 = (lane >> ((slot&3)*8)) & 0xFF`,
coverage in bits 1–7). All figures above use the byte-wise decode. The zoom 0.25/0.5/2 sweep was run only
under the bad decode and has NOT been redone — that item stays open.

## 2026-07-27 · P1 — the backward solve replaces the forward projection

`cardHit()` is now the sole occlusion predicate. **`shadowCover`, `projectTop` and `cross2` are deleted**
— no switch, no dual path, per [F6](forks.md#f6). Net **−59 / +48 lines** (`513b383`).

Nothing computes a forward projection anymore: every shadow decision in the renderer is a ray-vs-card
plane intersection.

### Verified

| check | result |
|---|---|
| corridor↔brute identity | **BIT-IDENTICAL** — 0 differing of 43 435 nonzero texels |
| renders | yes, at torch reach 8 — lights, shadows, silhouettes intact |
| frame time | 8.4 ms median / 119 fps (rAF), 60 fps in the debug panel (vsync) |
| forward-path references left | 0 |

### Behaviours carried across deliberately

- **`SHADOW_BASE_PUSH`** → a slightly negative `tMin`. `y` moves north as `t` rises, so south of the base
  is `t < 0`; the same seam-closing fudge, expressed in card coords instead of a corner position.
- **The `k <= 0` semi-infinite case** keeps its reach cap. Returning 0 there is exactly the bug I38 fixed,
  which silently deleted every shadow whenever a light was authored low. **This branch is the only place
  the reach bound is load-bearing** — [F2](forks.md#f2)'s general clamp is NOT reproduced, and identity
  held without it, which settles F2 in favour of option (c): presence already bounds which lights a texel
  sees.

### Known, recorded narrowing

The centre-ray gate is tighter than the forward projection it replaced — texels where the centre ray
misses but an offset sub-light hits (the penumbra) are now rejected. Shadows read slightly narrower than
the checkpoint. **P3 dissolves the gate rather than widening it**; do not "fix" this by re-broadening the
gate toward a forward projection.

### Not done in P1

- `worldTiltRad` is still fetched per caster per texel in `casterCover` despite being a pass-constant.
  Deleting `shadowCover` removed its duplicate fetch, which is half the win.
- Feeding the gate's `s`/`t` into the centre tap. Even tiers (2/4/8) have no centre sample, so the feed is
  only conditionally valid.
- P2's profile against `checkpoint/pre-plane-intersection` — frame time is measured but not A/B'd against
  the old build, so the miss-path win is still unquantified.

## 2026-07-27 · P2 profiling — INCONCLUSIVE, and the harness is the reason

**No performance claim is being made for P1.** Three attempts produced numbers that cannot all be right,
so the honest result is "not measured", not a figure.

| fixture | checkpoint | P1 |
|---|---|---|
| orbiting light, 5 s | 1.138 / 1.033 / **0.606** | 1.108 |
| frozen light + `rebakeAll()` each frame | 0.082 / 0.083 / 0.083 | — |

**Attempt 1 — orbiting light.** Gave checkpoint 1.138 vs P1 1.108 and I nearly reported a 2.6 % win.
Repeating the checkpoint gave 1.033 then 0.606 — a near-2× spread. The orbit phase carries ACROSS runs,
so each sample starts with the light somewhere different and a different caster set in range. The
variance swamps the effect by an order of magnitude; the comparison was meaningless.

**Attempt 2 — frozen light.** Tight (±0.001) but implausible: 0.083 ms for a full-window cold gather,
against ~1 ms measured on the same build minutes earlier. That is the signature of the bimodal harness
bug already recorded in [moving-lights](../2026-07-26-moving-lights/completed.md) — the wrapper times the
HOT pass, which is near-empty for a `hot 0` light, instead of the cold one. Draw-parity alone does not
pin which class you are timing.

### What a trustworthy P2 harness needs

- **Deterministic workload** — frozen light, fixed offset, forced rebake. Attempt 2 got this right.
- **Provably the COLD draw.** Not "every other draw": assert the class, e.g. by keying off the render
  target rather than a draw counter, and sanity-check that the figure MOVES when the workload obviously
  changes (zoom 1 → 0.5 quadruples the tiles; if the number does not move, the harness is lying).
- **A calibration case with a known answer** before trusting any A/B.

### Why the win may be small anyway

[moving-lights I11](../2026-07-26-moving-lights/issues.md) attributed `casterOne` at 70 % of frame with
the 16-tap emitter loop at 78 % OF THAT. The predicate is a thin slice of the cost; the tap loop is the
mass. So replacing the predicate was never going to move the frame much — **P3 is where the real win is**,
because it collapses 16 taps to 2. P1's justification stands on deleting a duplicated predicate and
halving a redundant texel fetch, not on a frame-time claim.

## 2026-07-27 · P3 — two rays replace the tap ladder

The N-tap emitter disk is gone. `casterCover` now solves TWO sub-lights at light ± emitter radius on the
caster's cross-axis and classifies from them:

| case | result |
|---|---|
| both rays hit the card | umbra — average their coverage |
| exactly one hits | penumbra — 0.5 for now; P4 fills the wedge from the two `s` values |
| neither hits, but they STRADDLE (opposite sides) | P is behind the caster between them → take the CENTRE ray |
| neither hits, same side | lit |

**The gate is gone, not optimised.** P1's centre-ray gate was explicitly transitional and rejected the
penumbra (centre misses, offset hits). With two rays the classification IS the test, so there is nothing
left to gate on, and the penumbra now extends past what the forward projection ever allowed.

**The straddle rule is what keeps trunks attached.** Close behind a caster the two extremes diverge past
opposite edges, so neither hits even though the point is solidly shadowed. Without the rule that reads as
a bright notch at every trunk. It is also the only case that costs a third solve — typical cost is two.

**A point light needs no special case.** `emitter == 0` makes the perpendicular offset zero, so both rays
ARE the centre ray and this degenerates to the exact hard shadow.

**DELETED:** `emitterOffset`, the adaptive tier selection, `uTapForce`, `uLadder`, and the `__taps` /
`__ladder` dials — with them go the graduated ladder, the odd/even centre-tap rule and the
`sqrt(n/ring)` radius compensation, all of which existed only to manage a cost that scaled with softness.

### Verified

| check | result |
|---|---|
| corridor↔brute identity | **BIT-IDENTICAL** — 0 differing of 40 737 nonzero texels |
| renders | yes, reach 8 — soft shadows, silhouettes intact, no trunk notches seen at zoom 1 |
| typecheck | clean |

### Snag: the backtick foot-gun bit again

I wrote a GLSL comment containing a name in backticks, which closed the `/* glsl */` template literal and
broke the build (`TS1005` at the function signature). The repo has a PostToolUse hook that catches exactly
this — but it matches `Write|Edit`, and I was editing via a python heredoc, so it never fired. Same reason
recorded in the memory index under "edit via Edit/Write, not bash". The hook cannot protect edits that
route around it.

## 2026-07-27 · P4 — analytic interval, and the first trustworthy measurement

The sampling schemes (16 taps → 2 rays → wedge → 4-tap binary search) were all hunting for the same
thing: WHERE on the emitter the shadow boundary falls. `u` is linear in the sub-light offset λ, so
inverting `u(λ) = u0 + λ·D` at `u = 0` and `u = 1` gives that boundary directly. Same plane intersection,
solved for the other unknown — the card coordinate already encodes its own edges as 0 and 1, so no edge
geometry and no trigonometry are involved. Clamping the resulting λ interval to the emitter's extent IS
the penumbra: fully inside → 1.0, half overhanging → 0.5, outside → 0.

    4 plane solves → 1     up to 4 texture fetches → 1     4 divides → 2

### MEASURED — one orbiting light, zoom 1, 240 frames, cold gather draw only

| reach (tiles) | checkpoint (forward + 16-tap) | analytic interval | speedup |
|---|---|---|---|
| 4 | 0.5589 ms | **0.1457 ms** | **3.84×** |
| 8 | 1.1189 ms | **0.3398 ms** | **3.29×** |
| 12 | 1.8960 ms | **0.5844 ms** | **3.24×** |

Dirty-tile counts matched across builds (110/263/404 vs 108/261/403), so the two were doing equivalent
work. Corridor↔brute BIT-IDENTICAL throughout (0 differing of 39 083).

### The harness — three bugs, and what actually fixed it

Every earlier attempt this session was untrustworthy, and the failures are worth keeping:

1. **Draw parity cannot identify the class.** Timing "every other gather draw" times cold or hot at
   random; the hot pass is near-empty for a `hot 0` light, which is how the SAME build measured 1.108 ms
   and 0.083 ms an hour apart. Fixed by discriminating on `o.target === coldShadowRT`.
2. **Accumulated orbit phase leaks across runs.** Each sample started with the light somewhere different,
   giving a 0.606–1.138 ms spread on one build. Fixed by deriving phase from the FRAME INDEX.
3. **Fixed duration ≠ fixed work.** Runs walked different arc lengths. Fixed by running a fixed frame
   count.

**And a calibration is mandatory before believing any figure.** The frozen-light harness looked
beautifully stable (±0.001) while being wrong — it did not move when zoom 1 → 0.5 quadrupled the tiles.
The orbit harness passes: cost tracks reach (0.146 → 0.340 → 0.584) and per-dirty-tile cost stays flat
(~0.0013 ms), and a repeat run lands within **0.2 %**. A harness that does not respond to a workload
change you control is measuring something else.
