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
