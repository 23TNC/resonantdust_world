# Plan — trait lights

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md),
decisions in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] VARIABLES.md: the `constant` assignment law (F2/F5/F6: TOML-only,
      immutable, zero-storage, thing-binds must be constant) + trait defs' new
      per-level `emit_light` parameter table (F4; row layout UNCHANGED — F3).
      Acceptance: docs-check green.

## P1 — the loader: constant binds, emit_light tables

- [x] TraitBindToml: `constant` (bool, default false) on binds; the resolved
      bind carries (name, level, constant). Acceptance: unit — a full bind
      parses; a bare string still means level 1, non-constant.
- [x] Trait defs: the per-level `emit_light` table (color, intensity, reach,
      fall_off, elevation, radius, flicker, cast/hot — F4/I2/I10), validated
      against the def's authored level count. Acceptance: unit — a def with 2
      light levels loads; a bind above the count refuses as today.
- [x] Load validation (F6): a NON-constant trait bind on a non-pawn thing
      refuses with a named error. Acceptance: a fixture thing with a
      non-constant bind fails load; a constant bind passes; goldens re-blessed
      deliberately (I6).

## P2 — the ONE merged-traits accessor

- [ ] shared/content: `object_traits(def, payload)` — the def's CONSTANT binds
      merged with payload rows, constant wins on collision (F5). Acceptance:
      unit — a constant bind appears with payload absent; a forged payload row
      for a constant-bound trait is ignored.
- [ ] `object_lights(def, payload)` beside it: every merged trait whose bound
      level authors emit_light yields its tuple, in authored order — NOTHING
      drops (F8). Acceptance: unit — torch yields one tuple matching its
      authored level; a def with five light traits yields five.
- [ ] Move ALL trait readers onto the accessor (I1): stat_eval, emotions,
      inventory capacity, food-chain caps, Tag checks, npc, webgl panels.
      Acceptance: grep shows no direct def-bind/payload trait merge outside
      the accessor; emotions goldens unchanged (I7).
- [ ] `mint_sidecars` skips constant binds (they derive; nothing mints).
      Acceptance: a spawned human's payload rows match pre-change (no bind is
      constant yet — I9); a test kind with a constant bind mints no row for it.

## P3 — the torch converts, the light block dies

- [ ] content: a light trait def authoring emit_light per level (level 1 =
      warm torch tuple, level 2 = blue; elevation = today's height 2.5,
      reach 16); torch + torch_blue bind it CONSTANT; `light =` blocks DELETED
      (F7). Acceptance: content-check + load green; no `light =` in content/.
- [ ] `thing_light()` composes its stride-8 output from `object_lights`;
      `LightToml` dies (F7/I3). Acceptance: the pre/post stride-8 vectors for
      both torches are IDENTICAL (unit fixture) — every downstream consumer
      untouched.
- [ ] Full consumer rebuild (I6): shared → wasm + webgl typecheck/build + npc
      + worker + master. Acceptance: builds green; live torch screenshot
      matches a pre-change capture (the bake drill, I3).

## P4 — pawns glow

- [ ] Movers whose merged traits yield lights attach light pieces at the CHASE
      position + authored elevation (I8); a TOML test bind on a pawn kind
      glows. Acceptance: screenshot; the light pans with a trip across a
      re-anchor; a torch-CARRYING pawn without the trait stays dark (I5).
- [ ] The SPILL packing (F8): billboards keep their authored depth slots;
      lights past the piece budget ride sibling light-only prims at the same
      carrier anchor, moving/despawning/classing with it. Acceptance: a test
      kind with 2 parts + 5 lights renders all five (light-count probe) and
      despawns clean; bake cost per extra light logged, not capped.

## P5 — the drills, and the truth

- [ ] Cold boot: bounce the stack; the world re-lights from cold rows + corpus
      alone (SQL shows no trait rows for torches). Acceptance: post-bounce
      screenshot matches; pawn payload row counts unchanged.
- [ ] Docs + memory truth pass + bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
