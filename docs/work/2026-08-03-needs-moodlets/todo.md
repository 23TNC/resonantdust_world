# Plan — needs & moodlets

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md)._

## P1 — the corpus speaks needs and moodlets

- [x] Add `&need.*` to `shared/dsl`: id/label, `deplete` (TICS full→empty, F4), and its moodlet
      BANDS (`lo`/`hi` satisfaction fractions). Acceptance: a thirst block round-trips in the
      loader's crate tests; an unauthored kind emits an empty need list.
- [x] Add `&moodlet.*`: label, `mood` offset (−1..1), `duration` tics (0 = conditional, F2).
      Acceptance: a loader test proves two moodlets bound to one need coexist without colliding.
- [x] Attach needs to kinds via `&thing.needs` (a list of need names). Acceptance: the wolf block
      lists thirst; the bundle exposes a per-kind need table the way `thingLayout` does.
- [x] Author `content/data/needs.rd`: thirst depleting (F1), Thirsty (sat < 0.35, mood −0.15) and
      Dehydrated (sat < 0.10, mood −0.40) — numbers PROVISIONAL but authored, never hardcoded.
      Acceptance: `bin/dsl` publishes; corpus parses in the crate tests.
- [x] Record the new layouts in `VARIABLES.md` (it outranks code). Acceptance: docs-check green;
      lanes documented before any consumer lands.

## P2 — the shard remembers

- [x] A pawn NEED lane via the `*_tables!` family: `(pawn, need_id) → (satisfaction, set_tic)`.
      Acceptance: `TABLES.md` row; the 2-pass native+wasm build green. → re-shaped by
      [F7](forks.md#f7): a `NEED` PAYLOAD opcode (`need_id:8|sat:8|set_tic:16`), not a new
      table — the sidecar is the documented home and its pipe already runs. TABLES.md
      documents the opcodes; 2-pass gate green.
- [x] A SET_NEED event verb: edge allowlist → queue → worker compose → state fan. Acceptance: a
      hand-issued SET_NEED lands in the shard and fans to a subscribed client — checked LIVE
      (subscription SQL is a string; the build gates can't see it). → verb 10 client-open;
      the pawn MODULE splices (worker relays). LIVE: `queue([10, wolf, 1, 25])` from the
      browser → payload row `NEED(1, 25, tic 1616)` in SQL. Client-side fan visibility rides
      P5's decode (the frame is the proven PART path, opcode-agnostic).
- [x] The STORED-moodlet lane `(pawn, moodlet_id, grant_tic, expiry_tic)` + GRANT_MOODLET verb —
      built now, exercised by the action stream (F2). Acceptance: `TABLES.md`; one hand event
      writes + fans; no gameplay consumer yet. → `MOODLET` opcode (expiry DERIVED from corpus
      duration, never stored); verb 11. LIVE: `queue([11, wolf, 3])` → `MOODLET(3, tic 1616)`
      beside the need entry, PART entries untouched.

## P3 — one evaluation, everywhere (F3)

- [x] `needs_eval` in a shared rust crate: `satisfaction_at(tic)`, active conditional moodlets,
      `mood = clamp(base + Σ offsets)`, `next_crossing_tic`. Acceptance: unit tests pin the band
      edges — the exact crossing tic, the clamp at 0, and an absent row. → `shared/dsl/needs_eval`
      (dep-free; rows in, moodlets out); 5 tests incl. sat==hi NOT active at the edge, tic wrap,
      timed expiry at exactly duration, band+grant stacking.
- [x] Expose the eval through `shared/wasm` for the client. Acceptance: the wasm call returns the
      identical moodlet set as the rust test fixture at three probe tics (full, mid-band, empty).
      → `pawnMoodlets`/`pawnMood`/`pawnNextCrossing` (raw payload in) + `moodletLabels`; browser
      probes vs hand-computed corpus values: full → mood 0.5/next 14041, mid → Thirsty 0.35/3262,
      empty → Dehydrated 0.10/−1, timed → Quenched remaining 3500. All exact.

## P4 — the wolf gets thirsty

- [x] npc initializes thirst at mint: SET_NEED (satisfaction 1.0, now-tic) chained after CREATE.
      Acceptance: an rd-npc soak shows one need row per minted wolf; re-mint stays one row.
      → adopt-time init with a 2 s snapshot grace: a restart against a wolf WITH a row held the
      guard (no reset — the drill row survived); the row deleted by hand → "thirst initialised
      FULL" fired once → SQL shows exactly one row `NEED(1, 255, set_tic 6052)`.
- [x] The Brain evaluates needs each decision tick (crate import) and surfaces moodlets + mood in
      its decision context, logging band transitions. Behaviour stays A→B — drink is the successor
      stream's. Acceptance: the soak log flips Thirsty→Dehydrated at the COMPUTED crossing tics
      with zero events in between (F4). → `mind_needs` in the wolves Brain (payload buffered
      per-entity pre-adoption; `Bot::now_tic` off the learned TicAnchor); first contact logged
      `[dehydrated] mood 0.1 next None` — the P2 drill row correctly drained. The full
      crossing-tic flip is P6's drill (the corpus rate is deliberately slow: full→Thirsty ≈ 39 min).

## P5 — the panel shows moodlets, never bars

- [x] Subscribe the need + stored-moodlet lanes for anchored pawns. Acceptance: selecting the wolf
      shows the rows client-side — checked LIVE (subscription SQL again). → F7 made this the
      EXISTING payload subscription; `MoverLayer.pawnPayload` holds the raw stream (pawnDefs
      lifecycle). LIVE: the browser reads `[131073, 33494948]` = `NEED(1, 255, 6052)` off the
      selected wolf — closing P2's fan-observation gap.
- [x] A MOODLETS section in the details panel: label + mood offset per active moodlet + the summed
      mood, evaluated lazily per frame via the wasm eval (TicEstimate for the now-tic). NO need
      bars anywhere — the abstraction is the feature. Acceptance: a forced-low SET_NEED makes
      Thirsty appear without a reload, and the band flip happens on screen at the crossing.
      → provider evaluates via `pawnMoodlets`/`pawnMood` at `ticDelta(0)` mod 2¹⁶; panel shows
      `mood NN%` + indented moodlet rows (timed grants append `Nt`). LIVE: forced sat 60 →
      "mood 35% / Thirsty −0.15" appeared without reload; the npc logged the SAME flip
      (`tic 7881, next 10804`) — two observers, one row, one eval.

## P6 — the drill

- [x] The crossing drill: SET_NEED to just above a band edge; the client panel and the npc log
      both flip AT the computed crossing tic. Acceptance: both sides' logged tics equal the
      computed one; zero need/moodlet events on the wire between set and flip. → sat 28
      (0.1098) set at tic 8378; computed crossing 8378+212 = 8590; npc flipped to Dehydrated
      at OBSERVED tic 8595 (its 1 s decision-tick granularity — the eval itself flips at
      exactly 8590, pinned by the unit tests + wasm probes); the payload row's tic stayed
      8378 throughout = ZERO writes/events between. Bonus fix: the sat-0 clamp no longer
      reports a spurious next-crossing (a bottom band holds forever; test added).
- [x] The lived soak: a wolf runs full → Dehydrated on a drill-scaled deplete rate while doing its
      normal trips; panel + npc agree throughout. Acceptance: captures + tics in `completed.md`;
      **the user's eyes close the stream**. → deplete drill-scaled to 1800 (reverted after):
      set FULL at tic 9253 → npc `[] 0.5 next 10424` → Thirsty observed 10425 (computed
      10424) `next 10874` → Dehydrated observed 10876 (computed 10874) `next None`; panel
      captured at 50% / 35% Thirsty / 10% Dehydrated; trips issued continuously around every
      flip; the payload row's tic stayed 9253 the whole arc — ONE write, zero events.
      Evidence in completed.md; the user's look is the remaining close.
