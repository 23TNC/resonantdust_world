# Forks — lighting

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Lightmap, not per-pixel forward {#f1}
**2026-07-23 — DECIDED (user): a baked LIGHTMAP.** Lighting accumulates into a cached, dirty-driven
lightmap RT (`lightmap = ambient + Σ per-light colour·falloff·diffuse·(1−shadow)`); the display
composites `albedo × lightmap` (warm-over-cold intact — [[rt-tiers-cold-warm-hot]]). NOT the per-pixel
forward blit — the lightmap recomputes only dirty regions (reuses the shadow dirty system), matches
the G-buffer map model, and is reusable by post (bloom). The `AlbedoBlitShader` display becomes the
`albedo × lightmap` composite.

## F2 · Falloff curve {#f2}
**2026-07-23 — open.** `intensity · f(dist/reach)`: smoothstep-to-0-at-reach (soft, cheap), inverse-
square-clamped (physical, needs a floor), or an artist curve. Lean smoothstep for the stylised look
([[art-style]]); tune by eye.

## F3 · Normal interpretation (top-down 2D + normal map) {#f3}
**2026-07-23 — open until P2.** The world is 3/4 top-down; normals are +Y-up OpenGL. Light dir per
pixel is in-plane (`light.xy − P`), but N has a z (up) component. Options: treat N·L with L lifted by
a fixed z (fake height) so flat ground (N≈up) gets ambient-ish and relief modulates; or a full 2.5D
dir using zdepth. Lean: in-plane L + fixed light-height z, tuned. Keep the wrap/2×−1 normal decode
consistent with the bake.

## F4 · HDR / tonemap {#f4}
**2026-07-23 — open until P4.** Dense lights sum > 1. Clamp (cheap, clips bright overlaps) vs a
tonemap (Reinhard/ACES — preserves colour in bright spots). Lean: clamp for P1–P3, add a cheap
tonemap in P4 if overlaps blow out.

## F5 · Lightmap resolution + where the normal is applied {#f5}
**2026-07-23 — open until P2.** Lighting has two frequencies: falloff + shadow are LOW-freq (a coarse
unit-res lightmap like shadow-cold, upsampled, is cheap + enough); normal RELIEF is HIGH-freq (per-px
art). Options: (a) **px-res lightmap** — bake `N·L` per-light into a full-res lightmap (full detail,
~16× the light math); (b) **coarse lightmap + per-px normal** — lightmap holds Σ(colour·falloff·
shadow) at unit-res (no normal) + an aggregate light direction; composite applies `N·L` per-px with
that aggregate (cheaper, loses per-light directionality). Decide by eye/perf at P2. Lean: start px-res
for correctness, drop to coarse if the light loop is too heavy.
