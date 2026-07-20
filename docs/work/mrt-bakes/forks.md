# Forks — mrt-bakes

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · Scratch-then-blit vs render direct into the channel buffers

- **4-attachment scratch, then blit per channel (chosen).** Mirror today's flow: MRT-render the square into a
  slot-sized 4-attachment scratch, then blit each attachment to its channel's slot **and its apron border**.
  Keeps the wrap-apron logic exactly as-is; only the "render" step changes (4 renders → 1). Cost: 4 blits
  (already the case).
- **Render direct into the four channel buffers (no scratch).** One MRT render straight to the slot in all
  four `fixedCW×fixedCH` buffers. Saves the scratch, but the **apron duplication** (edge slots copied to the
  opposite border) is a second write per edge slot — messier with MRT than a plain blit.

**PICK:** scratch-then-blit — smallest change to a proven hot path. _(pending)_

## F2 · Tier branch — one shader with a uniform, or two MRT shaders

- **One shader, per-prim uniform branch (chosen).** A `uMaterial` flag (or `uTier`) selects the material
  reconstruction path vs the flat-tint path inside the one MRT fragment. One program, one pipeline; the branch
  is per-draw-uniform (coherent, cheap).
- **Two MRT programs** (material-tier + flat-tier), pick per prim. Avoids the in-shader branch but doubles the
  program/pipeline and splits the batch by tier.

**PICK:** one shader + uniform branch, matching how the four separate bakes already resolve per prim. Revisit
only if the branch bloats the fragment. _(pending)_

## F3 · Keep the existing per-channel `Channel`/ping-pong structure?

- **Keep it (chosen).** The channels still own their `fixedCW×fixedCH` ping-pong buffers + display wiring;
  MRT only changes *how they're written* (one pass fills all four scratch attachments → blit to each
  channel's live buffer). `/overlayRT`, the display, LOD reproject all stay.
- **Collapse channels into one 4-attachment buffer set.** Bigger rewrite of the display/overlay/reproject
  paths for no extra bake win. Out of scope.

**PICK:** keep the channel structure; MRT is a write-path change only. _(pending)_
