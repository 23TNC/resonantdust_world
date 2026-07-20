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

- [ ] WebGL2 is already pinned (`preferWebGLVersion: 2`, done in `bitfield-rt`). Shaders are **GLSL
      ES 1.00** — Pixi's high-shader compiles ES 1.00 even on WebGL2 ([F14](forks.md#f14),
      [`design/rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md)); use
      **float-mod** for bits, not `uint`.
- [ ] **`shadow-hot`** — a **screen-space** float RGBA8 RT, viewport-sized (body px), `nearest`. RGB = 3
      lanes, A unused (float+blend → A avoided). Cleared + regenerated every frame; never persisted, never
      reprojected ([F8](forks.md#f8)).
- [ ] **`shadow-cold`** — a **world-space** bitfield RT in the SquareCache **toroidal** layout (share its
      window/slot geometry so it pans + scales with the other composites), **unorm RGBA8**, `nearest`.
      RED byte = the light-bitfield (6 bits this iteration, A=1 → premultiply is a no-op; widen to the full
      RGBA texel = 32 via the **verbatim non-premultiply write** to reclaim A — [F11](forks.md#f11),
      [I-5](issues.md#i-5), and the pattern `bitfield-rt` E1–E4 proved). **Filled by copy, not baked
      per-prim** — no per-rect dirty loop, just the window geometry + a nearest reproject on zoom.
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
- [ ] For each of the ≤4 quadrants, blit `shadow-hot` → `shadow-cold` through a **pack shader** (ES 1.00,
      **float-mod** — [I-6](issues.md#i-6)): read the frame's 3 lights from `shadow-hot` RGB + the **old**
      `shadow-cold` buffer, **OR in** those 3 lights' bits (float add into the batch's distinct bits),
      write the **new** buffer. **Ping-pong** (read old, write new — never sample the bound RT), per
      [I-8](issues.md#i-8); this is the mechanism `bitfield-rt` E5 proves. No blend.
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

- [ ] The **`OVERLAY_BITS`** decode mode already exists — `bitfield-rt` built + proved it in
      `overlayShader` (float-mod bit extraction on a unorm RGBA8, HSV palette, additive; `overlayModeFor`
      routes `lightmap`/`shadow`-style bitfields to it). For `shadows`, route `shadow-cold` to it and reuse
      as-is: 6 colours (RED) this iteration, up to 32 (full RGBA) at goal ([F5](forks.md#f5)).

## P7 · Verify the foundation — 2026-07-19

- [ ] In-browser at `?focus=100,50&coldlights`: a caster at the centre, `/overlayRT shadow-cold` → **6
      coloured shadows** fan out, overlaps combine. Move a light (`/coldlights` re-seed) → the shadow
      follows within a frame or two (round-robin) with no partial-shadow seams. **Zoom** in/out → shadows
      stay stuck to the world and scale cleanly.
- [ ] Confirm lossless packing: a spot shadowed by lights in different round-robin batches shows both
      colours (bits survive the copy + the other batches' writes).
