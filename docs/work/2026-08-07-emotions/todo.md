# Plan — emotions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [ ] VARIABLES.md: the `[[emotion]]` category (u4 declaration-order identity, `fine`
      first — F1), the `emotions` modifier lists on conditions/traits (F2), the u8
      pack (I3), the argmax law (F3), and MOOD's retirement note (F4). Acceptance:
      docs-check green.
- [ ] VARIABLES.md: the display laws — pie-square cards (F5), the everything-tooltip
      (F6), the panel wash (F7). Acceptance: docs-check green.

## P1 — the corpus

- [ ] Loader: `EmotionParams` (name/label/color; ≤16, fine-first, unique), condition
      `emotions` scalars + trait per-level arrays (magnitudes 1..15), ONE u8
      pack/unpack (I3); the `mood` FIELD deleted from the schema. Acceptance:
      round-trip + refusal unit tests green.
- [ ] Author the 16 emotions (user colors + F8 palette) and the standing conditions'
      emotions (thirsty/dehydrated/quenched — I5, mood lines deleted); golden
      re-blessed; six consumers rebuilt (I2). Acceptance: golden diff = the authored
      rows; edge hot-reload clean.

## P2 — the eval + the sweep

- [ ] `emotion_eval::active_emotion` (sums + argmax + fine/tie law — F3) with the
      user's worked example as a unit test (I6); the card sort's tie-break swapped to
      Σ magnitude desc in the ONE sort (I1). Acceptance: eval tests green incl. the
      3/5/2/6 → happy oracle.
- [ ] The mood SWEEP (F4/I1): `pawnMood` deleted; the `pawnConditions` stride's mood
      lane becomes Σ magnitude; new wasm accessors `pawnEmotion` (active id + color)
      and `conditionEmotions` (packed u8s + colors); npc + all five client files
      swept. Acceptance: grep `mood` clean outside history; all gates green.

## P3 — the display

- [ ] ConditionCards → pie-colored SQUARES: conic-gradient slices by magnitude in
      emotion-index order, solid for one modifier, gray for none, NO text (F5).
      Acceptance: a hand-authored +1 happy +2 sad condition renders 120°/240° (I4);
      captures.
- [ ] The everything-tooltip on hover (F6) + the panel-body wash in the active
      emotion's color, alpha-dimmed (F7). Acceptance: captures — the tooltip lists
      `+N <Emotion>` lines (+0 omitted, + always) and the wash tracks the active
      emotion.

## P4 — the verdict

- [ ] Drills: the user's oracle live (a test pawn granted 3 playful + 5 uncomfortable
      + 2 focused + 6 happy conditions → the wash turns happy's color); the standing
      arcs (thirst bands re-colored, wolf + human) run beside. Acceptance: captures +
      logs; the active emotion probe agrees with the eval.
- [ ] Docs + memory truth pass (needs-moodlets/stat-model memories note mood's
      retirement; index row records delivery) and a stack bounce with the arcs green;
      **the user's eyes close the stream**. Acceptance: docs-check green; captures +
      logs in completed.md.
