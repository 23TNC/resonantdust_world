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

## P3 · Dirty-tile gating (`shadow_dirty`) + persistence — 2026-07-21

- Replace the current **full recompute each frame** with a `discard`-gated single pass: a CPU
  dirty-tile set (light add/move/radius dirties its reached tiles; a caster move maps index → its
  lights → their reached tiles per F5; pan-exposed tiles dirty on recenter, cf. `SquareCache.markStale`)
  written into **`shadow_dirty`** (`R8UI` cols×rows, nonzero = dirty). The gather samples it and
  `discard`s clean tiles so `shadow-cold` persists across frames — **must be a picture no-op** (D-1).
- **Toroidal persistence caveat:** with `discard`, a tile's slot must be re-dirtied when the window
  pans onto a new world tile (else it shows the previous owner's bits). Mirror `SquareCache.markStale`.

## P6 · Budget + final verify — 2026-07-21

- Optional per-frame **budget**: cast only a subset of dirty tiles across frames; `log()` deferrals.
- Final verify: overlaps combine (✓ P4-core); **stable on pan** (needs P3 persistence); moving a light
  re-casts only its dirtied tiles.

---
_Done + verified → [`completed.md`](completed.md): **P1** (cold-texture reshape), **P4-core** (gather +
shadow-cold RT + bit-decode `/overlayRT` overlay = the P4 cast + P5 overlay), **P2** (`light_presence_cold`
per-tile cull). The `shadow-cold` RT alloc landed in P4-core._

_P1 (cold-texture reshape) done → [`completed.md`](completed.md). The `shadow-cold` RT allocation
moved into P4 (built with the gather that writes it)._
