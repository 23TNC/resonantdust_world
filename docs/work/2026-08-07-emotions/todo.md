# Plan — emotions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [x] VARIABLES.md: the `[[emotion]]` category (u4 declaration order, `fine` first —
      F1), condition/trait `emotions` modifiers (F2), the u8 pack (I3), the argmax
      law (F3), MOOD's retirement (F4). Acceptance: docs-check green. → the summary
      table + THE ACTIVE EMOTION law (argmax, fine, ties, the u8 pack, the worked
      example) + the sort's tie-break rewritten to Σ magnitude; the schema examples
      show `[[emotion]]`, condition `emotions`, trait per-level arrays; `mood` remains
      only in retirement notes; green.
- [x] VARIABLES.md: the display laws — pie squares (F5), the everything-tooltip (F6),
      the panel wash (F7). Acceptance: docs-check green. → the Emotion DISPLAY laws
      paragraph (conic pie from 12 o'clock, 120°/240° example, solid/gray degenerate
      cases, no card text, the tooltip contents, the alpha-dimmed wash); green.

## P1 — the corpus

- [x] Loader: `EmotionParams` (≤16, fine-first, unique), condition scalars + trait
      per-level arrays (magnitudes 1..15), ONE u8 pack (I3); `mood` deleted from the
      schema. Acceptance: round-trip + refusal tests green. → 42 lib tests green; the
      sort tie-break moved to Σ magnitude with it (the crate could not compile split).
- [ ] Author the 16 emotions (user colors + F8) and thirsty/dehydrated/quenched's
      emotions (I5, mood lines deleted); golden re-blessed; six consumers rebuilt
      (I2). Acceptance: golden diff = the authored rows; edge hot-reload clean.

## P2 — the eval + the sweep

- [ ] `emotion_eval::active_emotion` (sums, argmax, fine/tie law — F3) with the
      3/5/2/6 → happy oracle unit test (I6); the sort tie-break → Σ magnitude desc in
      the ONE sort (I1). Acceptance: eval tests green incl. the oracle.
- [ ] The mood SWEEP (F4/I1): `pawnMood` deleted, the stride's mood lane → Σ
      magnitude, new `pawnEmotion` + `conditionEmotions` wasm accessors, npc + client
      swept. Acceptance: grep `mood` clean outside history; gates green.

## P3 — the display

- [ ] ConditionCards → pie SQUARES: conic-gradient slices by magnitude, emotion-index
      order, solid for one, gray for none, NO text (F5). Acceptance: +1 happy +2 sad
      renders 120°/240° (I4); captures.
- [ ] The everything-tooltip (F6) + the alpha-dimmed panel wash (F7). Acceptance:
      captures — `+N <Emotion>` lines (+0 omitted, + always); the wash tracks the
      active emotion.

## P4 — the verdict

- [ ] Drills: the oracle live (3 playful + 5 uncomfortable + 2 focused + 6 happy
      grants → the wash turns happy); standing arcs beside. Acceptance: captures +
      logs; the probe agrees with the eval.
- [ ] Docs + memory truth pass (mood retirement noted; index row records delivery) +
      a stack bounce with arcs green; **the user's eyes close the stream**.
      Acceptance: docs-check green; captures + logs in completed.md.
