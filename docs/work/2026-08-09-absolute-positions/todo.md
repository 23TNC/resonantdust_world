# Plan — absolute-positions

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#), the migration rungs in [`ladder.md`](ladder.md)._

## P0 — the law and the paper

- [ ] VARIABLES.md: THE 48-BIT LAW named (F1, event_uid noted); the u64 position (F2,
      unit = the SIXTEENTH); the range-subscription form (F3); zones' fate (F4).
      Acceptance: docs-check green.
- [ ] The boundary lint (I8): the docs-check grep refusing `>>> 32`-class bitwise on u64
      carriers in TS. Acceptance: the lint fires on a planted violation; tree green.

## P1 — the spike (load-bearing, before anything hardens)

- [ ] A scratch table (realm/x/y btree + mover churn): live 2D band subs measured —
      latency at viewport counts, UPDATEs crossing edges (I2) — beside a per-zone
      baseline. Acceptance: numbers + the verdict (columns vs row-band) in completed.md.

## P2 — the codec core

- [ ] codec: the `position:u64` type (pack/unpack/tile/sixteenth/realm-guard I7) + range
      helpers (a viewport → its band predicate); legacy packers untouched beside it.
      Acceptance: unit round-trips + the law's asserts; realm != 0 refuses.

## P3 — the pilot: the pawn hot lane

- [ ] The pawn (+ player_pawn) shards gain absolute-position columns, DUAL-WRITTEN beside
      the legacy lanes (I3 — only the hot lane converts); worker writes both. Acceptance:
      republish green; live rows carry matching absolute + legacy positions.
- [ ] The edge serves ONE range subscription for movers; the client renders movers from
      it (the seam owns MOVERS only — I9); StateGone fans on band exit (I2). Acceptance:
      on camera — a pawn walks INTO the band and appears, OUT and vanishes.
- [ ] The mint→move→subscribe→render vertical drilled end to end on absolute positions
      (a host module's pawn preferred). Acceptance: captures + logs in completed.md.

## P4 — the ladder

- [ ] `ladder.md` finalized against what the pilot taught (rungs re-ordered/re-scoped with
      cause; each rung's dependency + acceptance sketch checked). Acceptance: the ladder
      stands alone — a fresh session could open rung 1 from it.

## P5 — the truth

- [ ] Docs + memory truth pass + stack bounce with arcs green (the zone-keyed world AND
      the pilot seam both live); **the user's eyes close the stream and pick the first
      rung**. Acceptance: docs-check green; captures + logs in completed.md.
