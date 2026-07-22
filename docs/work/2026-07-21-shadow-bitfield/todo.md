# Todo — 2026-07-21-shadow-bitfield

_Newest-first. Component: `client/webgl`. Design + assessment in [`README.md`](README.md); the
fixed-size layout + culling/update model in [`forks.md`](forks.md) (F6 consolidation, F5 presence,
F1 gather). The plan below reflects the **2026-07-21 fixed-layout redesign** — it supersedes the
earlier rect-accumulation phases (F3 dropped)._

_Remaining (execution order — the visible keystone P4-core + P5 overlay + P2 cull are done + verified;
what's left is optimisation + one refinement). Newest work at the bottom of this block, most-wanted first._

## P4-silhouette · Mask the fan region by the sprite silhouette — 2026-07-21

- The gather currently uses the **full fan region** (billboard outline → 5 triangles). Refine to the
  actual **sprite silhouette**: per in-region texel, inverse-map to the sprite UV and sample the
  surface `.B` coverage (the shared surface page, already captured by `ColdShadowData.surfacePage`);
  keep the bit only where covered. Visible (shadows take the tree shape, not a solid trapezoid).

## P6 · Budget (optional) — 2026-07-21

- Optional per-frame **budget**: cap dirty tiles cast per frame (mask a subset of `shadow_dirty` across
  frames), `log()` deferrals. Low priority — the gather is cheap at the current light/caster counts; add
  when a scene needs it. Core pan-stability already verified in P3.

---
_Done + verified → [`completed.md`](completed.md): **P1** (cold-texture reshape), **P4-core** (gather +
shadow-cold RT + bit-decode `/overlayRT` overlay = the P4 cast + P5 overlay), **P2** (`light_presence_cold`
per-tile cull), **P3** (dirty-tile gating + toroidal persistence — pan-stable). Deviation D-1 resolved.
The `shadow-cold` RT alloc landed in P4-core._
