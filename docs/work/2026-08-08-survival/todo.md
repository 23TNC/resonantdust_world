# Plan — survival

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md),
decisions in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] VARIABLES.md: `geo_label` on the thing schema (F1); the `deplete`
      DEPLETION MODIFIER on condition need-modifiers (F3 — the need's own
      field and units, rates summing, "only events and condition depletion
      move corpus"); the crossing re-stamp law (F4/I2 — one pending slot
      per (pawn, need), supersede, re-validate). Acceptance: docs-check
      green.

## P1 — geo glyphs

- [x] Loader: `geo_label` parsed (default = the name's first char,
      uppercased) and exposed beside the thing visual tables. Acceptance:
      unit — bunny → "B", an authored override wins; golden re-blessed.
- [x] Client: the glyph rasterizer (cached per label+bucket, white with a
      dark outline) drawn CENTERED on the geo-tier box AND the textureless
      placeholder box; gone the moment a real albedo serves (I5).
      Acceptance: a geo-tier capture shows B/L/D…; a loaded conifer shows
      none; the geo-flash law probe stays clean.

## P2 — the drain lane

- [x] shared/content: `deplete` on NeedModifier (conditions + the trait
      per-level form), rates summing, extended in the ONE piecewise eval
      with the trajectory unit test (I1); the panel's crossing forecast
      inherits it. Acceptance: the pinned trajectory test; existing goldens
      re-blessed deliberately.
- [x] content: `starving` and `dehydrated` author corpus depletion (F5 —
      ~3600 tics full→empty each; both active = twice the pace).
      Acceptance: load green; the wasm panel shows corpus falling for a
      starving pawn.
- [x] The consumer sweep (I1): wasm + webgl + npc + worker + master rebuilt
      and restarted on the new corpus. Acceptance: builds green; seed guard
      quiet.

## P3 — the crossing scheduler

- [x] Worker: after any write/mint/re-stamp touching a pawn's needs,
      compute the corpus zero-crossing (shared eval) and queue ONE
      re-validating SET_NEED re-stamp at that tic (+ barrier margin, I8
      horizon clamp); (pawn, need)-keyed ledger with supersede (I2) and the
      removal hook (I4). Acceptance: worker unit — a draining trajectory
      schedules once, a feed re-schedules later, a dead pawn unschedules.
- [x] The death hookup: the re-stamp at ≤ 0 triggers the EXISTING sweep →
      can_die → meat + remove; the attacked-while-starving race drilled
      (I3). Acceptance: live — a penned wolf's corpus hits 0, death fires
      ONCE, meat drops, StateGone fans.

## P4 — the drills

- [x] The starvation pen: a wolf denied food starves on camera (captures:
      the starving card, the falling corpus, the death, the meat); a bunny
      denied water dehydrates likewise. Acceptance: captures + logs in
      completed.md.
- [x] The control arc (I6): fed+watered wolf + warren survive a ≥30-min
      soak with population stable; humans asserted inheriting (a human's
      panel shows the drain while starving — no pen needed, I7 stated).
      Acceptance: the soak log + a panel capture.

## P5 — the truth

- [x] Docs + memory truth pass + stack bounce with arcs green; **the
      user's eyes close the stream**. Acceptance: docs-check green;
      captures + logs in completed.md.
