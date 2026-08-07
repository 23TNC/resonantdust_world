# Plan — human-pawns-redux

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), the anticipated-issue inventory in [`issues.md`](issues.md)._

## P0 — the paper trued

- [ ] Close [2026-07-30-pawn-part-placement](../2026-07-30-pawn-part-placement/README.md) as
      SUPERSEDED ([F5](forks.md#f5)): its README + the work-index row say why (the `pawns.rd`
      knobs are deleted; tuning is TOML now; the lighting/capture lanes carry as
      [I9](issues.md#i9)). Acceptance: `bin/rd work doctor` shows it closed; docs-check green.
- [ ] True the stale human-pawns paper: the work-index rows still naming `moveEntity` /
      `pawns.rd`, and `SquareCache`'s "human-pawns P5" doc notes vs the retired `layer` lane.
      Acceptance: grep `pawns.rd` clean outside completed/deviations history; docs-check green.

## P1 — the rot repairs (before anything new renders)

- [ ] The [subframe-ingest I11] fix ([F6](forks.md#f6)): MoverLayer registers per-SLOT
      subframes from `moverParts()`' rects against the stems `moverSlotTexture` builds
      (facing + variant-folder + part-suffix forms; west = mirrored east, never stored —
      [I1](issues.md#i1)). Acceptance: the wolf's e/s/n crops match its authored rects in the
      browser; subframe-ingest I11 marked closed with a pointer here.
- [ ] Humans gain `needs = ["thirst"]` ([F4](forks.md#f4)); golden re-blessed
      ([I6](issues.md#i6)). Acceptance: golden shows the rows; the drink drift is closed by
      construction (menu offer ⇔ worker accept, verified in P4's drill).
- [ ] Delete the retired `layer` write (MoverLayer + the `addPrim` copy site,
      [I8](issues.md#i8)) and outline EVERY part of a selected pawn in `syncOutlines`
      ([I4](issues.md#i4)). Acceptance: grep `layer:` gone from the mover path; a selected
      wolf still outlines correctly (the two-part check lands in P2's drill).

## P2 — the mint path

- [ ] The `spawn` chat command ([F2](forks.md#f2)): `spawn <thing> [x y] [body N] [head N]`
      composes `PROMOTE CREATE def pos count PART(0, body) PART(1, head)` via
      `part_entry`-shaped words + `queue`; variants validated against the manifest's ranges
      (9 bodies / 16 heads — [F3](forks.md#f3)); single-part kinds compose count 0.
      Acceptance: the command echoes its program; a refused variant says why.
- [ ] Drill the mint: `spawn human_female` near the pond — payload `[PART 0][PART 1][TRAIT…]`
      read back via sql (the worker appends the trait mint — the audit's pipeline), both
      prims render with the REGISTERED crops, the head depth-flips when facing north, and
      `pawnAt`/right-click selects with BOTH parts outlined. Acceptance: browser captures
      e/s/n; sql row in completed.md.

## P3 — variants

- [ ] Per-pawn appearance via the PART refs' variant nibbles ([F3](forks.md#f3)): the spawn
      command's `body`/`head` args select them; `moverSlotTexture`'s variant-folder probe
      resolves the art. Acceptance: `spawn human_male 103 64 body 2 head 7` renders variant-2
      body art under a variant-7 head.
- [ ] Drill two DIFFERENT females side by side (default vs chosen variants). Acceptance: one
      capture showing visibly different bodies/heads from one def.

## P4 — humans live on the current stack

- [ ] A human through the PIE MENU: right-click select (panel shows the derived
      `24 tics/tile` + mood/cards), Move To walks it (glide at 24), and at the water the menu
      offers Drink which the worker SATISFIES (+3 on its thirst row) — the F4 closure proven
      end to end. Acceptance: worker log `satisfied=Some("thirst")` for the human; captures.
- [ ] The standing wolf drills (trips, thermostat, panel) run unchanged beside the humans
      ([I1](issues.md#i1)'s wolf-crop check included). Acceptance: both species' arcs in one
      session's logs.

## P5 — the verdict

- [ ] Docs + memory truth pass: the human-pawns / pawn-render / subframe-ingest memories
      amended (I11 closed, the mint path exists, variants live), the stream index row
      records delivery. Acceptance: `bin/rd docs-check` green.
- [ ] Cold-boot the stack; `spawn` + variants + menu-driven move/drink + the wolf arcs green
      together ([I7](issues.md#i7): state what actually exists); **the user's eyes close the
      stream**. Acceptance: captures + logs in completed.md.
