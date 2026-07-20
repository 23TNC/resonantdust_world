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

## F2 · How the flat/geo tier is handled — NO branch; a "solid material"

**PICKED (user, 2026-07-20): unify every prim to ONE material descriptor; no per-prim branch or uniform.**
Instead of the MRT fragment (or the resolve) choosing a material path vs a flat-tint path, make the flat/geo
case a **degenerate material** — a "solid material": `residual = white`, `layers = null`, `surface = white`
(opaque/present/un-occluded), `normal = null` (→ flat-up), `tint = geoColor`, plus the tile depth. The
material reconstruction already handles tint and null layers, so a white residual × tint = the exact solid
colour the flat sprite produced today. Every prim then flows through the **one** material path; the MRT
fragment has a single code path.

Why this beats the branch:
- **One decision, not four.** Today each channel's `resolve` independently re-checks real-tier readiness
  (`alb.geo || surf.geo`) — the comments even worry about "a thing baking real in one channel but geo in
  another". Deciding solid-vs-real **once** per prim, producing one material that feeds all four outputs,
  makes that inconsistency impossible by construction.
- No `uMaterial` uniform, no batch split, no two programs.

Rejected: the per-prim uniform branch (my original draft) and two MRT programs — both keep the tier split
alive that the solid-material approach dissolves.

## F3 · Keep the existing per-channel `Channel`/ping-pong structure?

- **Keep it (chosen).** The channels still own their `fixedCW×fixedCH` ping-pong buffers + display wiring;
  MRT only changes *how they're written* (one pass fills all four scratch attachments → blit to each
  channel's live buffer). `/overlayRT`, the display, LOD reproject all stay.
- **Collapse channels into one 4-attachment buffer set.** Bigger rewrite of the display/overlay/reproject
  paths for no extra bake win. Out of scope.

**PICK:** keep the channel structure; MRT is a write-path change only. _(pending)_
