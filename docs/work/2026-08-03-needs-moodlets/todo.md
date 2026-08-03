# Plan — needs & moodlets

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md)._

## P1 — the corpus speaks needs and moodlets

- [ ] Add `&need.*` to `shared/dsl`: id/label, `deplete` (TICS full→empty, F4), and its moodlet
      BANDS (`lo`/`hi` satisfaction fractions). Acceptance: a thirst block round-trips in the
      loader's crate tests; an unauthored kind emits an empty need list.
- [ ] Add `&moodlet.*`: label, `mood` offset (−1..1), `duration` tics (0 = conditional, F2).
      Acceptance: a loader test proves two moodlets bound to one need coexist without colliding.
- [ ] Attach needs to kinds via `&thing.needs` (a list of need names). Acceptance: the wolf block
      lists thirst; the bundle exposes a per-kind need table the way `thingLayout` does.
- [ ] Author `content/data/needs.rd`: thirst depleting (F1), Thirsty (sat < 0.35, mood −0.15) and
      Dehydrated (sat < 0.10, mood −0.40) — numbers PROVISIONAL but authored, never hardcoded.
      Acceptance: `bin/dsl` publishes; corpus parses in the crate tests.
- [ ] Record the new layouts in `VARIABLES.md` (it outranks code). Acceptance: docs-check green;
      lanes documented before any consumer lands.

## P2 — the shard remembers

- [ ] A pawn NEED lane via the `*_tables!` family: `(pawn, need_id) → (satisfaction, set_tic)`.
      Acceptance: `TABLES.md` row; the 2-pass native+wasm build green.
- [ ] A SET_NEED event verb: edge allowlist → queue → worker compose → state fan. Acceptance: a
      hand-issued SET_NEED lands in the shard and fans to a subscribed client — checked LIVE
      (subscription SQL is a string; the build gates can't see it).
- [ ] The STORED-moodlet lane `(pawn, moodlet_id, grant_tic, expiry_tic)` + GRANT_MOODLET verb —
      built now, exercised by the action stream (F2). Acceptance: `TABLES.md`; one hand event
      writes + fans; no gameplay consumer yet.

## P3 — one evaluation, everywhere (F3)

- [ ] `needs_eval` in a shared rust crate: `satisfaction_at(tic)`, active conditional moodlets,
      `mood = clamp(base + Σ offsets)`, `next_crossing_tic`. Acceptance: unit tests pin the band
      edges — the exact crossing tic, the clamp at 0, and an absent row.
- [ ] Expose the eval through `shared/wasm` for the client. Acceptance: the wasm call returns the
      identical moodlet set as the rust test fixture at three probe tics (full, mid-band, empty).

## P4 — the wolf gets thirsty

- [ ] npc initializes thirst at mint: SET_NEED (satisfaction 1.0, now-tic) chained after CREATE.
      Acceptance: an rd-npc soak shows one need row per minted wolf; re-mint stays one row.
- [ ] The Brain evaluates needs each decision tick (crate import) and surfaces moodlets + mood in
      its decision context, logging band transitions. Behaviour stays A→B — drink is the successor
      stream's. Acceptance: the soak log flips Thirsty→Dehydrated at the COMPUTED crossing tics
      with zero events in between (F4).

## P5 — the panel shows moodlets, never bars

- [ ] Subscribe the need + stored-moodlet lanes for anchored pawns. Acceptance: selecting the wolf
      shows the rows client-side — checked LIVE (subscription SQL again).
- [ ] A MOODLETS section in the details panel: label + mood offset per active moodlet + the summed
      mood, evaluated lazily per frame via the wasm eval (TicEstimate for the now-tic). NO need
      bars anywhere — the abstraction is the feature. Acceptance: a forced-low SET_NEED makes
      Thirsty appear without a reload, and the band flip happens on screen at the crossing.

## P6 — the drill

- [ ] The crossing drill: SET_NEED to just above a band edge; the client panel and the npc log
      both flip AT the computed crossing tic. Acceptance: both sides' logged tics equal the
      computed one; zero need/moodlet events on the wire between set and flip.
- [ ] The lived soak: a wolf runs full → Dehydrated on a drill-scaled deplete rate while doing its
      normal trips; panel + npc agree throughout. Acceptance: captures + tics in `completed.md`;
      **the user's eyes close the stream**.
