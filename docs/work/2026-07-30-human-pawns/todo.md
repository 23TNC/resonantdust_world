# Todo — human-pawns

_Server truth first (packed defs + the payload exist in spacetime), then serving, then
DSL, then render, then the human itself, then graph conformance. Design stances:
[`README`](README.md); the payload/def redesign is user-directed ([F1](forks.md))._

---

## P0 — packed defs + the payload in spacetime

- [x] Repack pawn defs as `TYPE_PAWN | species | kind | variant` end-to-end: npc resolve
      packs it (species from the corpus), worker hop-cost keys on `def_kind_id`, wolves
      brain adoption compares kind not raw def, client decodes kind from the packed def.
      Acceptance: 2-pass build green; the npc soak logs a packed def wolf walking.
- [x] Add the SIDECAR tables `payload_log` + `payload` to the pawn shard (log/composed
      split; `payload` keyed by entity + zone for the subscription; opcode stream:
      `opcode:16|count:16` + operands; `PART = 1`, count 2: slot, def), SLAVED to state —
      no independent claim, writes only inside a state-write transaction; update
      `TABLES.md`. Acceptance: build green; TABLES.md shows both tables + the slaving
      rule + the encoding.
- [x] Keep movement hops payload-free: MOVE_TO/MOVE_STEP/PLACE never write the sidecar
      EXCEPT a zone-crossing write re-keying the `payload` row's zone under the same
      claim. Acceptance: a worker unit test — a same-zone hop leaves the sidecar
      byte-identical; a crossing hop changes only its zone key.
- [x] Grow `CREATE` to variable arity (`def · position · count · payload×count`) routing
      by `def_type_id` (TYPE_PAWN arm only; other types → a named rejection): spawn
      writes the first state row AND the payload sidecar in ONE transaction; worker arm,
      edge validation, npc emitters updated; update `ACTIONS.md` (CREATE = THE creation
      verb, per-type arms). Acceptance: build green; ACTIONS.md row updated; a non-pawn
      def CREATE logs the rejection.
- [x] Fan the `payload` sidecar through edge → protocol → core, joined to its entity's
      `StateObject` as a decoded `parts` list (`{slot, def}` per PART entry; unknown
      opcodes skipped by count; removal/StateGone drops the join). Acceptance: core unit
      test joins a 2-PART payload row to its state row.
- [x] Redeploy the pawn module + edge, then the LIVE check: CREATE a wolf with a marker
      payload, walk it a trip that CROSSES a zone, read the sidecar back. Acceptance: the
      payload survives byte-identically (zone key re-keyed); the wolf still walks at
      6 Hz.

## P1 — variant + part art served

- [x] Extend the edge stem grammar to `<base>/<variant>/<facing>.<part>` (variant default
      canonical `1`, part default `0`) in `textures.rs`. Acceptance: curl the edge for
      female variant 3 part 1 albedo → 200; existing wolf/conifer stems still 200.
- [x] Index variant folders + per-part maps in `tex_manifest` (per-leaf part list with its
      variant count) and regenerate `bin/art manifest`. Acceptance:
      `content/visual/manifest/pawn.rd` lists female/male with part 0 → 9 variants,
      part 1 → 16.
- [x] Accept the extended stems in the client resolver (fetch, preview, geo fallback).
      Acceptance: the debug page resolves a female body master AND its part-1 head master
      by stem (console `resolve` probe), both drawn from their own frames.

## P2 — DSL parts

- [x] Loader: read ALL `prims.N` into a parts list (`VisualParts` → parts vec) with new
      per-part fields `part` (default 0), `scale` (default 1, multiplier on part 0's
      size), `offset.x/y` (tiles, default 0). Acceptance: loader unit test over a 2-prim
      script reads both parts' fields.
- [x] Author the corpus: `human_female` / `human_male` appended to `content/data/things.rd`
      (speed) and a new `content/visual/pawns.rd` visual with body prim (span/size from the
      art) + head prim (`part 1`, `scale 0.625`, authored offset); fit/fat/average variant
      grouping recorded in comments. Acceptance: `bin/dsl` corpus parses; loader test
      resolves `human_female` to 2 parts with head scale 0.625.
- [x] Replace wasm `moverPrim` with `moverParts(kind)` returning the flat per-part array
      (stem, part, scale, offset, tint, geo per part) and update all callers — delete the
      old export. Acceptance: wasm build green; MoverLayer compiles against the new shape.

## P3 — client multi-part movers

- [ ] MoverLayer: a mover owns one warm prim PER PART slot from `moverParts`; part 0 boxes
      the carrier, other slots place at offset·tile and `scale`, zIndex just above part 0;
      each slot draws its payload `PART` def (stem + variant from the def), else the
      pawn's own def. Acceptance: the wolf (1 part, no payload) renders + walks
      identically in a soak; a 2-PART state row renders both parts.
- [ ] Move ALL part prims together in `tick` (speculation glide) and `applyVisual`.
      Acceptance: a drilled move shows no head/body lag (capture in `completed.md`).
- [ ] Take the wolf's sprite variant from `def.variant_id` (delete the id-derived pick in
      `thingTexture` callers). Acceptance: the wolf's variant is stable across reloads and
      equals the def's nibble.

## P4 — the first human in-world

- [ ] npc: spawn ONE STATIC human (`human_female`, e.g. body 7 head 11 as
      `PART(0,…)`/`PART(1,…)` CREATE payload) at a known location near spawn; the brain
      never issues MOVE — this IS the standing static drill fixture the user asked for.
      Acceptance: after `bin/sim run npc`, the human stands at the known tiles with the
      authored variants; restart adopts, never double-spawns.
- [ ] Joint drill at zoom 1 + 2: the human's body+head coherent (scale, offset, facing
      default s), lighting/shadows sane on both parts, wolf wandering past unaffected.
      Acceptance: captures in `completed.md`; the user's eyes are the final oracle.

## P5 — graph conformance (single carrier)

- [ ] Attach non-0 part billboards as carried pieces of part 0's carrier prim (free-slot
      claim, authored tile/unit offsets, child resolve stamps position; mover dirty
      cascades to child records). Acceptance: a record probe shows the human's carrier
      holding 2 billboard pieces; corridor↔brute shadow identity stays 0/0; the head's
      shadow tracks the body.
