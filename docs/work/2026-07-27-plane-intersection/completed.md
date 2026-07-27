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
