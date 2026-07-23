# Issues — unified data texture + scatter

_Problems hit + candidate solutions + which we chose + why. Chronological._

---

## v2 hard-freeze + the 1-fps overlay stall — RESOLVED 2026-07-23 {#angle-stall}

Two ANGLE/D3D11 pathologies hit while landing the scatter formats:

1. **v2 (address-block format) hard-froze the renderer** — main thread wedged inside GL (evals
   timed out). Prime suspect: the vertex shader's dynamic vector subscripts (`hdr[b/5]`,
   `vec[(rem>>1)&3]`) inside a break-loop — a known-fragile construct for the D3D shader compiler.
   Rewrote with static lane selection (`laneOf` ternaries); v2.1's simpler self-addressing shader
   ended the freezes. (The CPU fill loop was exonerated by node simulation: 1/22/1041 fills.)
2. **1 fps with `/overlayRT` on**: the OVERLAY — a full-screen **blended draw to the DEFAULT
   framebuffer** — sampling the scatter-written 16 MB data texture triggered a ~1 s/frame ANGLE
   stall (hazard-resolve per frame). The GATHER sampling the same texture into its own FBO is
   unaffected (121 fps). Fix: the overlay takes its window constants as **uniforms** (F2's own
   rule — it is a per-draw DISPLAY consumer) and no longer binds `uData`. Rule of thumb recorded:
   **don't sample the scatter-written table from default-framebuffer blended passes.**

