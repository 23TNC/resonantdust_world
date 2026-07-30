# Todo — human-pawns

_Server truth first (variants exist in spacetime), then serving, then DSL, then render,
then the human itself, then graph conformance. Design stances: [`README`](README.md)._

---

## P0 — variants in spacetime

- [ ] Widen the pawn shard to `entity_tables!(data: u16)`, layout
      `body_variant:4 | head_variant:4 | facing:2 | serial:6` (low byte bit-identical to
      today); update `TABLES.md`. Acceptance: 2-pass native+wasm build green; TABLES.md
      shows the layout.
- [ ] Mask the worker's serial/facing composition (MOVE_TO stamp, MOVE_STEP compare) to
      the LOW byte, preserving the variant byte. Acceptance: a codec/worker unit test
      round-trips serial ops over data `0xAB00`-style values unchanged in the high byte.
- [ ] Grow `CREATE` to arity 3 (`def · position · data` imm): spawn reducer writes `data`
      into the first row; worker arm, edge validation, npc emitters (wolves + wildlife)
      pass it; update `ACTIONS.md`. Acceptance: build green both passes; ACTIONS.md row
      updated.
- [ ] Fan `data:u16` through edge → protocol → core → `StateObject` as `bodyVariant` /
      `headVariant` fields (facing decode unchanged). Acceptance: core `state_event` unit
      test decodes a u16 row to facing + both nibbles.
- [ ] Redeploy the pawn module + edge, then the LIVE check: wolf CREATE with a nonzero
      variant byte, walk it one trip, read `entity_state.data` back. Acceptance: the
      variant byte survives the trip; the wolf still walks at 6 Hz.

## P1 — variant + part art served

- [ ] Extend the edge stem grammar to `<base>/<variant>/<facing>.<part>` (variant default
      canonical `1`, part default `0`) in `textures.rs`. Acceptance: curl the edge for
      female variant 3 part 1 albedo → 200; existing wolf/conifer stems still 200.
- [ ] Index variant folders + per-part maps in `tex_manifest` (per-leaf part list with its
      variant count) and regenerate `bin/art manifest`. Acceptance:
      `content/visual/manifest/pawn.rd` lists female/male with part 0 → 9 variants,
      part 1 → 16.
- [ ] Accept the extended stems in the client resolver (fetch, preview, geo fallback).
      Acceptance: the debug page resolves a female body master AND its part-1 head master
      by stem (console `resolve` probe), both drawn from their own frames.

## P2 — DSL parts

- [ ] Loader: read ALL `prims.N` into a parts list (`VisualParts` → parts vec) with new
      per-part fields `part` (default 0), `scale` (default 1, multiplier on part 0's
      size), `offset.x/y` (tiles, default 0). Acceptance: loader unit test over a 2-prim
      script reads both parts' fields.
- [ ] Author the corpus: `human_female` / `human_male` appended to `content/data/things.rd`
      (speed) and a new `content/visual/pawns.rd` visual with body prim (span/size from the
      art) + head prim (`part 1`, `scale 0.625`, authored offset); fit/fat/average variant
      grouping recorded in comments. Acceptance: `bin/dsl` corpus parses; loader test
      resolves `human_female` to 2 parts with head scale 0.625.
- [ ] Replace wasm `moverPrim` with `moverParts(kind)` returning the flat per-part array
      (stem, part, scale, offset, tint, geo per part) and update all callers — delete the
      old export. Acceptance: wasm build green; MoverLayer compiles against the new shape.

## P3 — client multi-part movers

- [ ] MoverLayer: a mover owns one warm prim PER PART from `moverParts`; part 0 boxes the
      carrier, other parts place at offset·tile and `scale`, zIndex just above part 0;
      variants from `StateObject` nibbles (part 0 ⇒ body, part 1 ⇒ head). Acceptance: the
      wolf (1 part) renders + walks identically in a soak; a human state row renders both
      parts.
- [ ] Move ALL part prims together in `tick` (speculation glide) and `applyVisual`.
      Acceptance: a drilled move shows no head/body lag (capture in `completed.md`).
- [ ] Take the wolf's sprite variant from `bodyVariant` (delete the id-derived pick in
      `thingTexture` callers). Acceptance: the wolf's variant is stable across reloads and
      equals the state value.

## P4 — the first human in-world

- [ ] npc: spawn ONE STATIC human (`human_female`, e.g. body 7 head 11, CREATE data) at a
      known location near spawn; the brain never issues MOVE — this IS the standing static
      drill fixture the user asked for. Acceptance: after `bin/sim run npc`, the human
      stands at the known tiles with the authored variants; restart adopts, never
      double-spawns.
- [ ] Joint drill at zoom 1 + 2: the human's body+head coherent (scale, offset, facing
      default s), lighting/shadows sane on both parts, wolf wandering past unaffected.
      Acceptance: captures in `completed.md`; the user's eyes are the final oracle.

## P5 — graph conformance (single carrier)

- [ ] Attach non-0 part billboards as carried pieces of part 0's carrier prim (free-slot
      claim, authored tile/unit offsets, child resolve stamps position; mover dirty
      cascades to child records). Acceptance: a record probe shows the human's carrier
      holding 2 billboard pieces; corridor↔brute shadow identity stays 0/0; the head's
      shadow tracks the body.
