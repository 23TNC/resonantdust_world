# Todo — shadows (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Phases run in
order — each verifiable on its own so a break is caught early. See [`README.md`](README.md) for the
architecture and [`forks.md`](forks.md) for the decisions._

---

## P0 · A minimal light source — 2026-07-19

`LightRig` was deleted in the nuke; this stream needs lights but NOT the old rig.

- [ ] A tiny light type — `{ x, y, z, radius }` in world px (no colour/brightness/tier; shadows don't
      need them yet) — held in a plain array on the viewport (not a rig class).
- [ ] Seed **6** around tile **(100, 50)** — a ring at ~2-tile radius so their shadows fan out in
      distinct directions from a caster at the centre. Via a `/coldlights [tileX tileY]` debug command
      (+ `?coldlights` URL), default (100, 50) ([F6](forks.md#f6)). Built to grow to the **24-light** goal.
- [ ] A caster enumerator (`standingPrims()` on `SquareCache`, `zIndex ≥ 1`) — the nuke removed it.

## P1 · The two RTs — 2026-07-19

- [ ] **`shadow-hot`** — a **screen-space** RGBA8 RT, viewport-sized (body px), `nearest`. RGB = 3 lanes,
      A unused. Cleared + regenerated every frame; never persisted, never reprojected ([F8](forks.md#f8)).
- [ ] **`shadow-cold`** — a **world-space** RGBA8 bitfield RT in the SquareCache **toroidal** layout
      (share its window/slot geometry so it pans + scales with the other composites), `nearest`, **A held
      at 1**. RED byte = the light-bitfield (6 bits this iteration; RGB/24 at goal) ([F9](forks.md#f9),
      [F11](forks.md#f11)). It is **filled by copy, not baked per-prim** — so it needs no per-rect dirty
      loop, only the window geometry + a nearest reproject on zoom.
- [ ] List `shadow-cold` in `Viewport.renderTextures()` so `/showRT` + `/overlayRT` see it. (`shadow-hot`
      is screen-space, so it isn't world-overlayable — expose it in `/showRT` only if useful for debug.)

## P2 · Screen-space billboard projection → `shadow-hot` — 2026-07-19

- [ ] For each of the **frame's 3 lights** and each in-range caster, project the caster's **billboard
      quad** (W×H, tilted by ground angle θ) radially to the ground per
      [`design/shadows.md` §Projection](../../components/client/pixijs/design/shadows.md), then map to
      **screen** px (world→screen via the current pan+zoom). Corners only — **no** 5-triangle fan, **no**
      per-corner depth, **no** UV ([D-3](deviations.md#d-3)).
- [ ] Draw each projected quad as **2 solid triangles** into `shadow-hot`, masked to its light's channel
      (frame-light 0→R, 1→G, 2→B) via `outColor = uChannel` + **`max` blend** (overlapping casters for one
      light clamp at 1). Clear `shadow-hot` each frame; a light with no in-range caster leaves its channel 0.

## P3 · The 4-copy pack: screen `shadow-hot` → world `shadow-cold` — 2026-07-19

- [ ] Compute where the screen rect maps in the **toroidal** world buffer — up to **4 wrapped quadrants**
      (straddling the H seam, the V seam, or both). Reuse the cache's existing window→buffer wrap math.
- [ ] For each of the ≤4 quadrants, blit `shadow-hot` → `shadow-cold` through a **pack shader**: read
      `shadow-hot` RGB (the frame's 3 lights) + the existing `shadow-cold`, **set** those 3 lights' bits,
      write back preserving the rest. A=1, non-premultiply, so the bitfield byte is exact ([D-1](deviations.md#d-1)).
- [ ] The 3 bits are chosen by the round-robin (P4): frame's batch `b` → bits `{3b, 3b+1, 3b+2}` (RED for
      the 6-light iteration).

## P4 · Round-robin cycling — 2026-07-19

- [ ] Each frame cast + copy the **next 3** lights (advance the batch cursor mod ⌈N/3⌉). 6 lights → 2
      frames to fill; the 24-light goal → **8 frames** (~130ms) ([F12](forks.md#f12)).
- [ ] When the light set changes (a `/coldlights` re-seed / a light moves), reset the cursor and **clear**
      `shadow-cold` so stale bits don't linger (mirrors the nuked build's invalidate-on-light-change).

## P5 · Zoom scaling of `shadow-cold` — 2026-07-19

- [ ] On zoom, `shadow-cold` reprojects/scales with the cache window **like the other RTs but `nearest`**
      (bilinear garbles a bitfield). The every-frame `shadow-hot` regeneration then refills at the new
      scale, so there are no shadow-specific zoom artifacts beyond a few frames of round-robin catch-up
      ([I-1](issues.md#i-1)).

## P6 · The `/overlayRT shadow-cold` bit-decode — 2026-07-19

- [ ] A **shadow-bits** overlay mode: route `shadow-cold` → it in `overlayModeFor`.
- [ ] In `overlayShader`, decode the bits (`bf_bit` float-mod, ES-1.00 safe:
      `mod(floor(byte*255 / 2^i), 2.0)`), sum each set bit's unique colour into the output — 6 colours
      (RED bits) this iteration, 24 (RGB) at goal, **additive** so overlaps combine ([F5](forks.md#f5)).
      Transparent where no bit is set.

## P7 · Verify the foundation — 2026-07-19

- [ ] In-browser at `?focus=100,50&coldlights`: a caster at the centre, `/overlayRT shadow-cold` → **6
      coloured shadows** fan out, overlaps combine. Move a light (`/coldlights` re-seed) → the shadow
      follows within a frame or two (round-robin) with no partial-shadow seams. **Zoom** in/out → shadows
      stay stuck to the world and scale cleanly.
- [ ] Confirm lossless packing: a spot shadowed by lights in different round-robin batches shows both
      colours (bits survive the copy + the other batches' writes).
