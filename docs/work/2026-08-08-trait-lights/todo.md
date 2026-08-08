# Plan — trait lights

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md),
decisions in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [ ] VARIABLES.md: the trait row's data half becomes `data:8 | level:8` (F3 —
      trait family ONLY; needs/conditions keep u16) + the `constant` assignment
      law (F2/F5/F6: TOML-only, immutable, zero-storage, thing-binds must be
      constant). Acceptance: docs-check green.

## P1 — the codec split

- [ ] Codec: `pack_trait_data(data, level) -> u16` + `trait_row_level` /
      `trait_row_data` accessors beside the gameplay-row helpers (F3).
      Acceptance: unit — round-trip both bytes; a legacy u16-level row (data 0)
      reads its level unchanged.

## P2 — the loader: constant binds, thing traits, emit_light authoring

- [ ] TraitBindToml: `data` (u8, default 0) + `constant` (bool, default false)
      on binds; the resolved bind carries (name, level, data, constant).
      Acceptance: unit — a full bind parses; a bare string still means level 1,
      data 0, non-constant.
- [ ] Load validation (F6): a NON-constant trait bind on a non-pawn thing
      refuses with a named error; constant binds validate level against the
      def's authored count exactly like today. Acceptance: a fixture thing with
      a non-constant bind fails load; the torch's constant bind passes.
- [ ] Trait defs gain the per-level LIGHT table + presentation residue (F4/I2):
      `emit_light` authors color per level + intensity/radius/height/flicker/
      cast/hot once. Acceptance: loader exposes the table; golden re-blessed
      deliberately (I6).

## P3 — the ONE merged-traits accessor

- [ ] shared/content: `object_traits(def, payload)` — the def's CONSTANT binds
      merged with payload rows, constant wins on collision (F5). Acceptance:
      unit — a constant bind appears with payload absent; a forged payload row
      for a constant-bound trait is ignored.
- [ ] Move ALL trait readers onto the accessor + the split accessors (I1):
      stat_eval, emotions lookup, inventory capacity, food-chain caps, npc,
      webgl panels. Acceptance: `grep gameplay_row_data` shows no trait-row
      caller; emotions goldens unchanged (I7).
- [ ] `mint_sidecars` skips constant binds (they derive; nothing mints) and
      packs (data, level) via `pack_trait_data` for the rest. Acceptance: a
      spawned human's payload rows match pre-change (no bind is constant yet —
      I9); a test kind with a constant bind mints no row for it.

## P4 — emit_light replaces the light block

- [ ] content: `emit_light` trait def (level 1 = the warm torch color, level 2
      = blue, residue per I2); torch + torch_blue bind it CONSTANT with
      data = 16; `light =` blocks DELETED (F7). Acceptance: content-check +
      load green; no `light =` remains in content/.
- [ ] `thing_light()` composes its stride-8 output from the constant
      emit_light bind + the def's residue table; `LightToml` dies (F7/I3).
      Acceptance: the pre/post vectors for both torches are IDENTICAL (unit
      fixture), so every downstream consumer is untouched.
- [ ] Full consumer rebuild (I6): shared → wasm + webgl typecheck/build + npc
      + worker + master. Acceptance: builds green; live torch screenshot
      matches a pre-change capture (the bake drill, I3).
- [ ] Pawn hot light (I8): movers whose merged traits carry emit_light attach
      a light piece at the CHASE position; a TOML test bind on a pawn kind
      glows and the light follows a trip. Acceptance: screenshot + the light
      pans with the mover across a re-anchor; a torch-CARRYING pawn without
      the trait stays dark (I5).

## P5 — the drills, and the truth

- [ ] Cold boot: bounce the stack; the world re-lights from cold rows + corpus
      alone (no spacetime trait rows for torches — SQL shows none).
      Acceptance: post-bounce screenshot matches; pawn payload row counts
      unchanged.
- [ ] Docs + memory truth pass + bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
