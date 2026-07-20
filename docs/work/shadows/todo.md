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

## P1 · Platform + the two RTs — 2026-07-20

- [ ] **Pin WebGL2** in the renderer init (don't let Pixi silently fall back to WebGL1, which would break
      the ES 3.00 shaders) and author the shadow shaders as **`#version 300 es`** ([F14](forks.md#f14),
      [`design/rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md)).
- [ ] **`shadow-hot`** — a **screen-space** float RGBA8 RT, viewport-sized (body px), `nearest`. RGB = 3
      lanes, A unused (float+blend → A avoided). Cleared + regenerated every frame; never persisted, never
      reprojected ([F8](forks.md#f8)).
- [ ] **`shadow-cold`** — a **world-space** bitfield RT in the SquareCache **toroidal** layout (share its
      window/slot geometry so it pans + scales with the other composites), `nearest`. Prefer an **integer
      `RGBA8UI`** target (real `uint` bitwise, A usable → 32 bits) — or a unorm RGBA8 with verbatim
      non-premultiply writes if the Pixi plumbing for integer RTs is fiddly ([F11](forks.md#f11),
      [I-5](issues.md#i-5)). RED byte = the light-bitfield (6 bits this iteration; full RGBA/32 at goal)
      ([F9](forks.md#f9)). **Filled by copy, not baked per-prim** — no per-rect dirty loop, just the window
      geometry + a nearest reproject on zoom.
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
- [ ] For each of the ≤4 quadrants, blit `shadow-hot` → `shadow-cold` through a **pack shader**
      (`#version 300 es`): `texelFetch` the existing `shadow-cold` bits + read `shadow-hot` RGB (the frame's
      3 lights), **OR in** those 3 lights' bits (`bits |= mask << shift`), write the `uvec4` back. No blend
      (read-modify-write); real `uint` bitwise, so no `n/255` float-mod ([I-6](issues.md#i-6)).
- [ ] The 3 bits are chosen by the round-robin (P4): frame's batch `b` → bits `{3b, 3b+1, 3b+2}` (RED byte
      for the 6-light iteration; spilling into G/B/A as the count grows toward 32).

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
- [ ] In `overlayShader` (`#version 300 es`), decode the bits with real bitwise — `(bits >> i) & 1u` — and
      sum each set bit's unique colour into the output: 6 colours (RED bits) this iteration, up to 32 (full
      RGBA) at goal, **additive** so overlaps combine ([F5](forks.md#f5)). Transparent where no bit is set.
      (If `shadow-cold` is an integer `usampler2D`, the overlay samples it with `texelFetch`.)

## P7 · Verify the foundation — 2026-07-19

- [ ] In-browser at `?focus=100,50&coldlights`: a caster at the centre, `/overlayRT shadow-cold` → **6
      coloured shadows** fan out, overlaps combine. Move a light (`/coldlights` re-seed) → the shadow
      follows within a frame or two (round-robin) with no partial-shadow seams. **Zoom** in/out → shadows
      stay stuck to the world and scale cleanly.
- [ ] Confirm lossless packing: a spot shadowed by lights in different round-robin batches shows both
      colours (bits survive the copy + the other batches' writes).
