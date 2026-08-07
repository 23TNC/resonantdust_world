# Plan — human-pawns-redux

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the paper trued

- [x] Close [2026-07-30-pawn-part-placement](../2026-07-30-pawn-part-placement/README.md) as
      SUPERSEDED ([F5](forks.md#f5)): its README + the work-index row say why (the `pawns.rd`
      knobs are deleted; tuning is TOML now; the lighting/capture lanes carry as
      [I9](issues.md#i9)). Acceptance: `bin/rd work doctor` shows it closed; docs-check green.
      → the README banner + the index row (also keeping its still-real I1 blit-footprint
      finding named); doctor shows `closed`; docs-check green.
- [x] True the stale human-pawns paper: the work-index rows still naming `moveEntity` /
      `pawns.rd`, and `SquareCache`'s "human-pawns P5" doc notes vs the retired `layer` lane.
      Acceptance: grep `pawns.rd` clean outside completed/deviations history; docs-check green.
      → the misleading OPEN references were pawn-part-placement (closed above) and the
      human-pawns index row (a since-delivery note appended: TOML corpus, dead mover, redux
      re-opens the mint). Remaining `pawns.rd` hits are DONE streams' historical records —
      kept (rewriting history files is the wrong fix). The SquareCache `layer` doc-note is
      deleted WITH its write in P1 (one change, not two).

## P1 — the rot repairs (before anything new renders)

- [x] The [subframe-ingest I11] fix ([F6](forks.md#f6)): MoverLayer registers per-SLOT
      subframes from `moverParts()`' rects against the stems `moverSlotTexture` builds
      (facing + variant-folder + part-suffix forms; west = mirrored east, never stored —
      [I1](issues.md#i1)). Acceptance: the wolf's e/s/n crops match its authored rects in the
      browser; subframe-ingest I11 marked closed with a pointer here. → `subframeSlot` beside
      `scaleSlot` (the [variant][rotation] cell, rot = `facing === 3 ? 1 : facing`);
      `MoverPart.subframes` finally READ; `Viewport.setSubframe` passthrough. LIVE: resolver
      holds `wolf/e#0 [0, 0.2578, 1, 0.4688]` and — the facing the cold path never covered —
      `wolf/n#0 [0.3359, 0.0859, 0.3281, 0.8438]`, both authored-verbatim. I11 closed with
      the pointer; south + part-suffix stems register on first occurrence (same one path).
- [x] Humans gain `needs = ["thirst"]` ([F4](forks.md#f4)); golden re-blessed
      ([I6](issues.md#i6)). Acceptance: golden shows the rows; the drink drift is closed by
      construction (menu offer ⇔ worker accept, verified in P4's drill). → both kinds; golden
      diff = ONE line (the stride-8 needs table gains `0x80010010` in both human slots);
      content-check clean.
- [x] Delete the retired `layer` write (MoverLayer + the `addPrim` copy site,
      [I8](issues.md#i8)) and outline EVERY part of a selected pawn in `syncOutlines`
      ([I4](issues.md#i4)). Acceptance: grep `layer:` gone from the mover path; a selected
      wolf still outlines correctly (the two-part check lands in P2's drill). → the write,
      the copy, the `Primitive.layer` field and its P5 doc-note all gone (the retirement
      pointer lives on `carrierOf`'s doc); syncOutlines now walks `partPrimIdsOf` (see P2's
      drill for the two-part proof; the wolf outline re-verified there).

## P2 — the mint path

- [x] The `spawn` chat command ([F2](forks.md#f2)): `spawn <thing> [x y] [body N] [head N]`
      composes `PROMOTE CREATE def pos count PART(0, body) PART(1, head)` via
      `part_entry`-shaped words + `queue`; variants validated against the manifest's ranges
      (9 bodies / 16 heads — [F3](forks.md#f3)); single-part kinds compose count 0.
      Acceptance: the command echoes its program; a refused variant says why. → landed;
      SOFTENED (recorded): validation is the u4 HARD bound (0..15) — the client has no clean
      per-part art-count accessor, and a missing variant folder degrades visibly through the
      existing stem fallback rather than refusing; x/y default to the camera centre.
- [x] Drill the mint: `spawn human_female` near the pond — payload `[PART 0][PART 1][TRAIT…]`
      read back via sql (the worker appends the trait mint — the audit's pipeline), both
      prims render with the REGISTERED crops, the head depth-flips when facing north, and
      `pawnAt`/right-click selects with BOTH parts outlined. Acceptance: browser captures
      e/s/n; sql row in completed.md. → `0x30800003` payload
      `[PART 0 0xA0][PART 1 0xA0][TRAIT bio@1][TRAIT walks@1]` via sql; both prims render
      seated (captures); right-click selects with BOTH parts silhouette-outlined; the panel
      shows `pawn/human/female (#10) · 24 tics/tile · resting`. The north depth-flip rides
      P4's walk (a static south-facing mint can't show it). NOTE: this mint pre-dated the
      worker's corpus reload, so it carries NO thirst row — the worker loads its bundle at
      startup; fresh mints after the restart carry it (the P3 female does).

## P3 — variants

- [x] Per-pawn appearance via the PART refs' variant nibbles ([F3](forks.md#f3)): the spawn
      command's `body`/`head` args select them; `moverSlotTexture`'s variant-folder probe
      resolves the art. Acceptance: `spawn human_male 103 64 body 2 head 7` renders variant-2
      body art under a variant-7 head. → sql: the male's payload holds `0x300200B2`/
      `0x300200B7` (the nibbles verbatim) and renders the broader body-2; the variant
      subframe stems registered themselves (`female/5/s#0`, and the PART-SUFFIX stem
      `female/12/s.1#0` — I11's last uncovered form, now live).
- [x] Drill two DIFFERENT females side by side (default vs chosen variants). Acceptance: one
      capture showing visibly different bodies/heads from one def. → captured: the HEAVY
      body-5/head-12 female (the spec's fat class, outline-confirmed) beside the slim
      default female and the broad male — three silhouettes from two defs.

## P4 — humans live on the current stack

- [x] A human through the PIE MENU: right-click select (panel shows the derived
      `24 tics/tile` + mood/cards), Move To walks it (glide at 24), and at the water the menu
      offers Drink which the worker SATISFIES (+3 on its thirst row) — the F4 closure proven
      end to end. Acceptance: worker log `satisfied=Some("thirst")` for the human; captures.
      → 0x30800005: panel `24 tics/tile · mood` + Thirsty/Quenched CARDS; menu Move To →
      worker `move_to dest="(102, 68)"`; her own tile offered `["Drink","Move To"]` → worker
      `drink satisfied=Some("thirst") from=18.62 to=21.62 grants=1`. Glide sampled mid-walk
      NORTH at 0.22–0.24 tiles/s tracking the derived 0.267 (6.41 tics/s ÷ 24 — not the old
      16). The P2-deferred depth-flip landed on that walk: facing 2, head `female/12/n.1`
      z=66.6566 UNDER body `female/5/n` z=66.6665 (south neighbour: head z 67.0101 OVER body
      67) + the zoomed capture. Drill note: a HIDDEN tab freezes rAF (chase/spec rendering)
      while WS state flows — env artifact, recorded in issues.md.
- [x] The standing wolf drills (trips, thermostat, panel) run unchanged beside the humans
      ([I1](issues.md#i1)'s wolf-crop check included). Acceptance: both species' arcs in one
      session's logs. → one session, interleaved: wolf 0x30800002 thermostat drinks at
      01:19/01:22/01:26 (`satisfied=Some("thirst")` 34.7→37.7, band 25 + `quenched`
      re-crossings) between the human's 01:23 `move_to` and 01:11 drink; 714 trip arrivals;
      panel: `pawn/animal/wolf (#7) · 12 tics/tile · moving · mood 70% · Quenched +0.20`;
      I1 crop check: east-facing mid-trip capture shows the authored wide side rect
      (1.0×0.4688), outline tracking — no letterbox, no shift.

## P5 — the verdict

- [x] Docs + memory truth pass: the human-pawns / pawn-render / subframe-ingest memories
      amended (I11 closed, the mint path exists, variants live), the stream index row
      records delivery. Acceptance: `bin/rd docs-check` green. → audited: NO memory claimed
      the stale facts (the I11s in other memories are different streams' ledgers; "human"
      appeared nowhere) — the amendment is the new `human-pawns-redux-delivered` memory +
      index line (mint door, variant nibbles, I11 closure, depth-flip, worker-bundle-at-
      startup gotcha, I10 drill artifact); the work-index row flipped done with the
      delivery note; docs-check green (4 standing warnings, none ours).
- [x] Cold-boot the stack; `spawn` + variants + menu-driven move/drink + the wolf arcs green
      together ([I7](issues.md#i7): state what actually exists); **the user's eyes close the
      stream**. Acceptance: captures + logs in completed.md. → sim crates stopped + edge
      killed + client reloaded; master resumed the DURABLE clock (worker composing at 12855
      on boot); all 6 pawns re-fanned at their exact pre-boot tiles; `/spawn human_male 101
      65 body 8 head 3` minted 0x30800006 (nibbles 0x300200B8/B3 verbatim, worker sidecars:
      2 PART + 2 TRAIT + full thirst row); menu Move To → arrival → menu `["Drink","Move
      To"]` → `drink satisfied=Some("thirst") 18.81→21.81 grants=1`; wolf trips resumed
      (01:36 arrivals). I7 stated: 7 pawns exist — 3 wolves + 4 humans, of which 03/04
      (pre-worker-restart mints) still carry NO thirst rows (recorded dev dirt). Captures:
      the b8/h3 male at the pond, both parts outlined (a one-frame texture-fetch race shows
      rect fallbacks on the very first frame — resolves on the next). The user's eyes close
      the stream on this report.
