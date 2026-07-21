# Completed — mrt-bakes

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## B1 · MRT spike — 4 outputs into a 4-attachment target — 2026-07-20

`es300MrtSpike.ts` (`/mrttest`): a raw ES 3.00 mesh whose fragment writes four distinct colours to
`layout(location=0..3) out`, rendered into a `RenderTarget({ colorTextures: 4 })`, shown as a 2×2 grid.
Verified: four different colours (red/green/blue/yellow) → all four attachments written; the scene behind is
unaffected. **Key finding ([I-1](issues.md#i-1)): Pixi never calls `gl.drawBuffers`, so MRT needs a manual
`gl.drawBuffers([COLOR_ATTACHMENT0..3])` after binding the target's FBO** — per-FBO state, so it sticks and
doesn't pollute the screen. That's the one non-native piece the B4 bake must carry.

## B2 · Universal material — every prim through one path — 2026-07-20

The albedo `resolve` now returns a material for EVERY prim: real → reconstruction (output tint white);
flat/geo → a **solid material** (residual × `uTint`=geoColor, a WHITE surface so the shader never discards
→ full box, no layers). Added `uTint` (output multiply) to `materialBakeShader` — white for real (no-op,
tint lives in chA/chB), geoColor for solid — and `materialNode` sets it. No more flat-sprite path for
albedo; one bake path. Verified: ground tiles, real trees (green) and geo-tier boxes all bake identically
(steady-state pixel-match; the I-8 tint trap handled by the white-for-real rule).
