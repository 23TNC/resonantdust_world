# Forks — lighting

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Where the lighting pass runs {#f1}
**2026-07-23 — open until P1.** Extend `AlbedoBlitShader`'s display draw (sample albedo+normal+
surface+presence+shadow-cold, output lit) vs a separate lighting RT composited after. Extending the
blit is fewer passes; a separate RT is cleaner for future post (bloom). Warm-over-cold compositing
([[rt-tiers-cold-warm-hot]]) must survive either way. Lean: extend the blit for P1, split later if post needs it.

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
