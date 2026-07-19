# Todo — lighting

_Phased so the renderer keeps working after each step. Items move to [`completed.md`](completed.md) as
they land + verify. Design (authoritative):
[`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md) · this work
[`README`](README.md) · decisions [`forks.md`](forks.md) · deps [`blockers.md`](blockers.md)._

_Shipped ([`completed.md`](completed.md)): the G-buffer tiers, cold `lightmap-cold` bake (32 uniform
lights + an **interim** RGB=3 `shadow-cold`), `projectCaster`, the scatter-shader shell. What's below
converges those onto the [design](../../components/client/pixijs/intent/tiered-lighting.md): cold 3→32
shadow bitfield, the whole warm/rt dynamic path, retire the wedge._

---

## P1 · Foundation — ✅ DONE (`a37be74`, see completed.md)

The `add`→`max` fix + the ported bitfield helpers landed. Remaining foundation item folds into P3:
- [ ] **Verify the 4th (alpha) lane** end-to-end (`uChannel` + `max` + non-premult writeback → 4
      lanes/map) — only testable once the ScatterPass renders into lanes; do it there. Sets throughput.

## P2 · Cold upgrade — 3 → 32 shadow-casters (bake-time)

- [ ] **Per-rect cold light-data texture** (port `coldLightTex.ts`): 2 texels/light (`xy`,`z`,`radius`;
      colour,brightness), `alphaMode: no-premultiply-alpha`, `nearest`. The cold bake reads its rect's
      ≤32 lights from this texture instead of `uLightData[32]` uniforms (each rect's nearest 32 differ).
- [ ] **`shadow-cold` as a 32-bit bitfield** — built **on dirty** via the shared scatter engine (32 lights
      = 8 scatter passes @ 4 lanes → ping-pong writeback into the bitfield). Rect-aware over all casters
      within reach.
- [ ] **Cold bake reads the bitfield** (`bf_bit`) not the RGB lanes → every cold light in a rect casts a
      shadow. `sum += light·N·L·atten·(1 − occludedBit·STRENGTH)`.
- [ ] **Remove the interim RGB=3 path** — the old `uColdShadow` 3-lane sample + `bakeColdShadowSquare`'s
      RGB lanes (keep the scatter machinery, now feeding the bitfield). [D-1](deviations.md).
- [ ] Verify live: >3 cold shadow-casters on one rect ALL cast.

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
