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

## B3 · The merged 4-out MRT bake shader — 2026-07-20

`mrtBakeShader.ts`: ONE ES 3.00 fragment writing all four channels — attachment 0 = albedo (material
reconstruction × tint, via the high-shader template's `finalColor`), 1 = surface, 2 = normal, 3 = depth.
Reuses the material reconstruction + OKLab; the shared surface map gives BOTH the coverage/discard AND the
surface output. **I-9 resolved:** the template's `finalColor` has no explicit location, which ES 3.00
forbids alongside `layout(location=1..3)` — patched the assembled fragment source to `layout(location = 0)
out vec4 finalColor;` before it compiles (kept ALL the high-shader transform plumbing; no raw program
needed). Surface presence R = `tileDepth >= 0 ? 1 : 0` reproduces the old per-case R (real 1 / ground 0)
byte-identically.

## B4 · bakeSquare — one MRT render — 2026-07-20

`bakeSquare` now builds one `mrtNode` per prim (gathering the albedo/normal/depth resolves), renders the
square ONCE into a 4-attachment `RenderTarget` scratch (with the manual `gl.drawBuffers` per I-1), then
blits each attachment to its channel's slot + apron. Replaced the per-channel loop (4 renders → 1). The old
per-channel node methods are now dead (follow-up cleanup).

## B5 · Verified — 2026-07-20

Every channel pixel-identical via `/overlayRT albedo-cold|surface-cold|normal-cold|zdepth-world-cold`
(green trees/ground; cyan-ground/white-things surface; magenta normals; blue depth silhouettes). Albedo
display, `/shadowcast`, and pan strip-rebake all correct. No new compile errors. Four G-buffer bakes
collapsed to one MRT pass.
