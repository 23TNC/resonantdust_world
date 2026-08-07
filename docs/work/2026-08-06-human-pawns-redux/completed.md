# Completed — human-pawns-redux

## 2026-08-06 — P0: the paper trued

- **pawn-part-placement closed as SUPERSEDED** ([F5](forks.md#f5)): its README banners the
  why (the `pawns.rd` knobs are deleted, tuning is TOML; the lighting/capture lanes carry as
  [I9](issues.md#i9)) and its still-real I1 blit-footprint finding stays named. Verified:
  `bin/rd work doctor` shows it closed; docs-check green.
- **Stale human-pawns paper trued**: the work-index row gained a since-delivery note (TOML
  corpus, dead mover, redux re-opens the mint). Remaining `pawns.rd` greps are DONE streams'
  historical records, kept deliberately. Verified: grep clean outside history; docs-check
  green.

## 2026-08-06 — P1: the rot repairs

- **Subframe-ingest I11 closed** ([F6](forks.md#f6)): `subframeSlot` in `applyVisual`
  registers each slot's authored `[variant][rotation]` rect (rot = `facing === 3 ? 1 :
  facing` — west is mirrored east, never stored) under the RESOLVED stem;
  `MoverPart.subframes` is finally read; `Viewport.setSubframe` passes through. Verified
  LIVE in the resolver: `wolf/e#0 [0, 0.2578, 1, 0.4688]`, `wolf/n#0 [0.3359, 0.0859,
  0.3281, 0.8438]` — authored-verbatim; I11 marked closed with the pointer.
- **Humans thirst** ([F4](forks.md#f4)): both human kinds gain `needs = ["thirst"]`; golden
  re-blessed — the diff is ONE line (both human slots' stride-8 needs table gains
  `0x80010010`). Verified: content-check clean; the menu-offer ⇔ worker-accept closure
  proven in P4's drill.
- **`layer` lane deleted + all-parts outline**: the MoverLayer write, the `addPrim` copy,
  the `Primitive.layer` field and its doc-note are gone (retirement pointer on `carrierOf`);
  `syncOutlines` walks `partPrimIdsOf`. Verified: grep clean; two-part outline in P2's
  drill; the wolf outline re-verified in P4.

## 2026-08-06 — P2: the mint path

- **The `spawn` chat command** ([F2](forks.md#f2)): `spawn <thing> [x y] [body N] [head N]`
  composes `PROMOTE CREATE def pos count PART(0,body) PART(1,head)` via `queue`, echoing the
  program; variants validated to the u4 bound (SOFTENED from manifest-ranges — recorded in
  the item: no clean per-part art-count accessor client-side; a missing folder degrades
  through the stem fallback); x/y default to the camera centre.
- **The mint drilled**: `0x30800003` payload read back via sql — `[PART 0 0xA0][PART 1
  0xA0][TRAIT bio@1][TRAIT walks@1]` (the worker appends the trait mint); both prims render
  with REGISTERED crops; right-click selects with BOTH parts outlined; panel shows
  `pawn/human/female (#10) · 24 tics/tile · resting`. NOTE: this mint pre-dated the worker's
  corpus reload so it carries no thirst row — the worker loads its bundle at startup; the
  fresh P3/P4 mints carry it.

## 2026-08-06 — P3: variants

- **Variant nibbles live** ([F3](forks.md#f3)): `spawn human_male 103 64 body 2 head 7` →
  sql payload holds `0x300200B2`/`0x300200B7` verbatim; the broader body-2 renders; the
  variant-folder stems self-register (`female/5/s#0`, and the PART-SUFFIX form
  `female/12/s.1#0` — I11's last uncovered shape).
- **Two females side by side**: captured — the heavy body-5/head-12 female beside the slim
  default female and the broad male: three silhouettes from two defs.

## 2026-08-07 — P4: humans live on the current stack

- **A human through the pie menu, end to end** (0x30800005): panel `24 tics/tile` + mood +
  Thirsty/Quenched cards; menu Move To → worker `move_to dest="(102, 68)"`; on her own tile
  the menu offered `["Drink","Move To"]` → worker `drink satisfied=Some("thirst")
  from=18.62 to=21.62 grants=1` (+3 exact, the F4 closure). Glide sampled mid-walk NORTH:
  0.22–0.24 tiles/s tracking the derived 0.267 (6.41 tics/s ÷ 24 t/t — not the retired
  authored 16). The P2-deferred depth-flip landed on that walk: facing 2, head
  `female/12/n.1` z=66.6566 UNDER body `female/5/n` z=66.6665; south neighbour head
  z=67.0101 OVER body z=67 — plus the zoomed capture (body contour overlapping the head's
  lower third). Drill artifact recorded as [I10](issues.md#i10): a hidden tab freezes rAF
  (spec/chase presentation) while WS state flows — diagnosis cost, not a code bug.
- **The wolf arcs beside the humans, one session**: wolf 0x30800002 thermostat drinks at
  01:19/01:22/01:26 (`satisfied=Some("thirst")` 34.7→37.7 at the 25-band, `quenched`
  re-crossings logged by the npc) interleaved with the human's 01:23 `move_to`; 714 trip
  arrivals; wolf panel `pawn/animal/wolf (#7) · 12 tics/tile · moving · mood 70%`; I1 crop
  check: east-facing mid-trip capture shows the authored wide side rect with the outline
  tracking — no letterbox, no shift.

## 2026-08-07 — P5: the verdict

- **Truth pass**: no memory claimed the stale facts (the I11s elsewhere are other streams'
  ledgers) — the amendment is the new `human-pawns-redux-delivered` memory + index line;
  the work-index row flipped done with the delivery note; docs-check green (4 standing
  warnings, none this stream's).
- **Cold boot green**: sim crates stopped, edge killed, client reloaded → master resumed
  the DURABLE clock (worker composing at tic 12855 straight from boot), all 6 pawns
  re-fanned at their exact pre-boot tiles, npc adopted 0x30800000 and resumed trips + the
  thirst drill. Post-boot: `/spawn human_male 101 65 body 8 head 3` (the slash prefix IS
  the command grammar) minted 0x30800006 — echoed program carries `0x300200B8`/`0x300200B3`
  verbatim; sql shows the worker's appended sidecars (2 PART + 2 TRAIT entries + a full
  thirst row at tic 13704). Menu Move To walked him to the pond (worker 01:35:53), the menu
  on his tile offered `["Drink","Move To"]`, and the worker satisfied
  (`Some("thirst") 18.81→21.81 grants=1` on the seeded row — lazily drained then +3 exact).
  Wolf trips arrived at 01:36 beside him. **I7 stated — what actually exists**: 7 pawns (3
  wolves, 4 humans); humans 0x30800003/04 (minted before the worker's corpus reload) still
  carry NO thirst rows — recorded dev dirt, re-mint or a full wipe clears it. Captures: the
  b8/h3 male at the pond with both parts silhouette-outlined; his very first presented
  frame showed rect fallbacks (texture-fetch race, resolved next frame — the hidden-tab
  one-frame-per-capture cadence of I10 made it visible at all).
