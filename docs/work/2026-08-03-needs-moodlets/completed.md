# Completed — needs & moodlets

## 2026-08-03 · P1 — the corpus speaks needs and moodlets (5/5)

`shared/dsl` gains the `<need>` + `<moodlet>` registries (material-style: direct `@define`,
1-based append-stable ids), `NeedParams { label, deplete, bands }` with up to 4
`NeedBand { moodlet, lo, hi }` slots, `MoodletParams { label, mood, duration }`
(duration 0 = conditional, F2), and `&thing.needs.<0..7>` → `thing_needs()` /
`thing_needs_table()` (stride 8, 0 = empty — the `thingLayout` shape). Indexed slots use
bare digits (the `packed.<i>` precedent; the I8 hazard needs a scalar sibling to bite,
documented as "don't add one").

Corpus: `content/data/needs.rd` authors thirst (deplete 21600 tics = 1 h wall at 6 Hz;
bands Thirsty [0.10, 0.35) −0.15, Dehydrated [0, 0.10) −0.40, EXCLUSIVE) + timed
Quenched (+0.20, 3600 tics) so the timed form parses from day one; the wolf's
`:data @define` carries `"thirst &thing.needs.0 set`. `VARIABLES.md` gains the
"Needs & moodlets" section.

**Verified**: 3 new loader tests (registry round-trip with two bands coexisting on one
need; unauthored need → name-label/0-deplete/no-bands; wolf resolves thirst + typo slot
drops + table stride) and the REAL-corpus smoke test extended (thirst registered, drains,
2 bands all naming registered moodlets, wolf carries it) — 50 tests green in the
spacetime docker image. (Note: the docker `test` service trips on a stale
host-glibc codec test binary; `cargo test -p resonantdust-dsl` is the clean lane.)
