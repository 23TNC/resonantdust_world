# Todo — shadows (newest-first)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. The phases
run in order — each is verifiable on its own so a break is caught early. See
[`README.md`](README.md) for the slice and [`forks.md`](forks.md) for the decisions._

---

## P0 · A minimal cold-light source — 2026-07-19

`LightRig` was deleted in the nuke; this stream needs lights but NOT the old rig.

- [ ] Add a tiny cold-light type — `{ x, y, z, radius }` in world px (no colour/brightness/tier; shadows
      don't need them yet) — and a plain array of them owned by the viewport (not a rig class).
- [ ] Seed **6** around tile **(100, 50)** — a ring at ~2-tile radius so their 6 shadows fan out in
      distinct directions from a caster at the centre. Seed via a `/coldlights [tileX tileY]` debug chat
      command (+ `?coldlights` URL), defaulting to (100, 50) — mirrors the removed `/coldlight`
      ([F6](forks.md#f6)).
- [ ] Re-add a caster enumerator (`standingPrims()` on `SquareCache`, `zIndex ≥ 1`) — the nuke removed it.

## P1 · The two RTs + the `shadow-cold` channel — 2026-07-19

- [ ] Add **`shadow-cold`** as a cold-tier `SquareCache` composite (world-space, per-rect, dirty-baked, so
      it aligns with the world-space `/overlayRT` display geometry). RGBA8, `nearest`, **A held at 1**.
- [ ] Add **`shadow-hot`** as a per-rect **scratch** RT (RGBA8, `nearest`) sized to one slot — reused
      across batches/rects, not a persisted display channel.
- [ ] Re-introduce a per-rect **post-pass hook** in `SquareCache.bakeSquare` (the nuke removed the derived
      channel + `bakeColdShadowSquare`) that runs the project→stage→pack loop below into `shadow-cold`.
- [ ] List `shadow-cold` in `Viewport.renderTextures()` so `/showRT` + `/overlayRT` see it.

## P2 · Billboard shadow projection → `shadow-hot` channels — 2026-07-19

- [ ] For a caster's billboard quad (W×H, tilted by the ground angle θ) and a light `L`, project the 4
      corners to the ground with the radial formula in
      [`design/shadows.md` §Projection](../../components/client/pixijs/design/shadows.md) — corners only,
      **no** 5-triangle fan, **no** per-corner depth, **no** UV ([D-3](deviations.md#d-3)).
- [ ] Draw the projected quad as **2 solid triangles** into `shadow-hot`, with each of the batch's 3
      lights masked to its own channel (light 0→R, 1→G, 2→B) via a per-light channel-write shader
      (`outColor = uChannel`) + **`max` blend** so overlapping casters for one light clamp at 1
      ([F2](forks.md#f2), and the tiered-lighting anti-goal "no `add` blend").
- [ ] Clear `shadow-hot` before each batch; skip a light that has no in-range caster (its channel stays 0).

## P3 · Pack `shadow-hot` RGB → `shadow-cold` RED bits — 2026-07-19

- [ ] Seed the rect's `shadow-cold` RED byte to 0 (A=1) before batch 0.
- [ ] Combine pass per batch: read `shadow-hot` RGB + the prev `shadow-cold`, **OR in** three bits —
      batch `b` writes bits `3b+0/1/2` with values `2^(3b+k)/255`. `red' = prevRed + present·bitVal`
      (each pass writes distinct bits, so a plain add is an OR). Keep A=1; write verbatim/non-premultiply
      so the RED byte is exact ([D-1](deviations.md#d-1)). Ping-pong `shadow-cold` between the two batches.
- [ ] After batch 1, `shadow-cold` RED holds all 6 bits. Blit to the composite slot (+ wrap-apron, like
      the other channels).

## P4 · The `/overlayRT shadow-cold` bit-decode — 2026-07-19

- [ ] Add a **shadow-bits** overlay mode: route `shadow-cold` → it in `overlayModeFor`.
- [ ] In `overlayShader`, decode the RED byte's 6 bits (`bf_bit` float-mod, ES-1.00 safe:
      `mod(floor(red*255 / 2^i), 2.0)`), and sum each set bit's unique colour into the output —
      6 distinct colours, **additive** so overlaps combine ([F5](forks.md#f5)). Drop to transparent
      where no bit is set (the scene reads through).

## P5 · Verify the foundation — 2026-07-19

- [ ] In-browser at `?focus=100,50&coldlights`: place a caster at the centre, `/overlayRT shadow-cold`,
      confirm **6 coloured shadows** fan out in 6 directions and **overlaps combine colours**. Move the
      caster / re-seed lights; confirm the bitfield re-bakes (dirty) and the overlay tracks it.
- [ ] Confirm the packing is lossless: a spot shadowed by lights 0 and 3 shows exactly colour 0 + colour 3
      (bits in different batches survive the ping-pong).
