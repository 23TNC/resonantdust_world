# Todo — lighting

_Phased so the renderer keeps working after each step. Items move to [`completed.md`](completed.md) as
they land + verify. Design (authoritative):
[`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md) · this work
[`README`](README.md) · decisions [`forks.md`](forks.md) · deps [`blockers.md`](blockers.md)._

_Shipped ([`completed.md`](completed.md)): the G-buffer tiers; cold `lightmap-cold` bake (32 uniform
lights); the **RGB=3 interim `shadow-cold`** — now **solid** (buffer-grow: casts ALL trees in range,
≤3 cold lights, browser-verified); `projectCaster`; the ported bitfield building blocks
(`bitfield.ts`, `warmCombineShader`, `coldLightTex`). What's below lifts that onto the
[design](../../components/client/pixijs/intent/tiered-lighting.md): cold 3→32 bitfield, the warm/rt
dynamic path, retire the wedge. **The RGB=3 interim is the working fallback until the bitfield lands.**_

---

## P1 · Foundation — ✅ DONE (`a37be74`, see completed.md)

The `add`→`max` fix + the ported bitfield helpers landed. Remaining foundation item folds into P3:
- [ ] **Verify the 4th (alpha) lane** end-to-end (`uChannel` + `max` + non-premult writeback → 4
      lanes/map) — only testable once the ScatterPass renders into lanes; do it there. Sets throughput.

## P2 · Cold 3 → 32 bitfield — RE-ATTEMPT (first try reverted; root cause known — [issues.md I1](issues.md))

**Do it stage-by-stage with read-back verification** — the first attempt (`14f4b0b`) was built end-to-end
blind and produced an empty `shadow-cold`. Root cause found: the **Sprite-based `blit()` premultiplies**
the field (`RGB × A`), and a bitfield texel with `A = 0` gets its bits zeroed. The interim survived only
because its scratch cleared to `A = 1`.

- [ ] **Stage A — one lane round-trips.** Render ONE cold light's silhouettes into a scatter lane; **read it
      back** (a bright coverage blit to `shadow-cold`, `overlayRT` it) and confirm the lane holds coverage.
- [ ] **Stage B — combine → bits.** Feed the lane through `warmCombineShader` into a 32-bit field; **read
      back** the field and confirm the expected bit is set.
- [ ] **Stage C — field reaches the composite.** Blit the field to `shadow-cold` with a **non-premultiply
      Mesh copy** (`blendMode "none"`, like the old game's `warmMesh`), NOT the Sprite `blit()`. Also confirm
      the `shadow-cold` **composite** isn't premultiplied. `overlayRT` → confirm the bits survive to it.
- [ ] **Stage D — bake reads it.** `lightingBakeShader` reads `bf_bit(bf_byte(csh, i/8), i%8)`; confirm the
      shadow renders (dark wedge in the lit pool).
- [ ] **Then the full loop:** 4 batches @ 8 lanes → all 32 bits; the square-level light-reach cull; the
      `nBatches`-by-count + buffer-grow (already in the interim) carried over.
- [ ] **Per-rect cold light-data texture** (`coldLightTex.ts`, ported): `alphaMode: no-premultiply-alpha`,
      `nearest`; bake reads its rect's ≤32 lights from it instead of `uLightData[32]` uniforms.
- [ ] **Remove the interim RGB=3 path** ONLY once the bitfield is browser-verified. [D-1](deviations.md).
- [ ] Verify live: **>3 cold shadow-casters on one rect ALL cast** (the whole point).

## P3 · Warm dynamic path (currently 0% — the wedge fakes it)

- [ ] **`ScatterPass`** — each frame, gather the round-robin batch (4 warm lights), run `projectCaster`
      per caster within radius, rasterize into **`shadow-hot`** (4 lanes). Adapt `Viewport.buildCasters`
      to yield `{feetX, groundY, footNY, h, w, left, stem}` + `OutlineCache.get(stem)`.
- [ ] **`shadow-warm` 32-bit bitfield + ping-pong writeback** — combine `prev shadow-warm` + the 4 fresh
      `shadow-hot` lanes → `next shadow-warm` with those 4 bits set (deferred writeback at frame top).
      4/frame → 8-frame refresh.
- [ ] **Display loop** — restructure the 32-light sum: each light gated by its occlusion — the 4 fresh via
      their `shadow-hot` lane (zero-lag), the other 28 via `shadow-warm` `bf_bit`; **cross-fade** the fresh
      4 (`0.5·(warmBit+freshLane)`) so catch-up is a 1-frame fade. `uFreshChannel`-style uniform names the
      batch.

## P4 · RT priority (zero-staleness)

- [ ] **`shadow-rt`** — 4 highest-priority lights rasterized **every frame** into their own 4-lane map
      (never round-robin, never written to a bitfield). Display reads the lane directly.
- [ ] Migrate the **cursor** light into the rt set.

## P5 · Retire the stopgaps

- [ ] Delete the wedge [`shadowPass.ts`](../../../client/pixijs/src/game/viewport/shadowPass.ts) + the
      single global `uShadow` multiply in the display. Verify multi-light: two casters, B lighting A's spot
      does NOT un-shadow it. (Reconcile the `nsProject` axis when the caster geometry is next touched —
      [D-2](deviations.md).)

## P6 · Content + cleanup

- [ ] **DSL static lights** — author cold point lights in content; classify light→tier at worldgen/load
      ([B2](blockers.md#b2)).
- [ ] Graduate this README's residue into `design/lighting.md` (the intent doc is already the target).

---

**Done when:** `lit = albedo × (lightmap_cold + Σ₃₂ warm·N·L·!occ + Σ₄ rt·N·L·!occ)` — cold baked with
**32** shadow-casters/rect, 32 warm on a round-robin bitfield, 4 rt always-fresh, no cross-contamination,
the wedge + RGB=3 cap deleted, browser-verified.
