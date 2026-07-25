# Todo — 2026-07-21-shadow-bitfield

_Newest-first. Component: `client/webgl`. Design + assessment in [`README.md`](README.md); the
fixed-size layout + culling/update model in [`forks.md`](forks.md) (F6 consolidation, F5 presence,
F1 gather). The plan below reflects the **2026-07-21 fixed-layout redesign** — it supersedes the
earlier rect-accumulation phases (F3 dropped)._

_The stream is functionally **complete + browser-verified** (P1 → P4-core → P2 → P3 → P4-silhouette).
Only an explicitly-optional budget remains._

## P6 · Budget (optional, deferred) — 2026-07-21

- [ ] Optional per-frame **budget**: cap dirty tiles cast per frame (mask a subset of `shadow_dirty` across
  frames), `log()` deferrals. **Low priority — deferred**: the gather is cheap at the current
  light/caster counts and pan-stability is already verified (P3). Revisit if a dense scene spikes.

---
_Done + verified → [`completed.md`](completed.md): **P1** (cold-texture reshape), **P4-core** (gather +
shadow-cold RT + bit-decode `/overlayRT` overlay = P4 cast + P5 overlay), **P2** (`light_presence_cold`
per-tile cull), **P3** (dirty-tile gating + toroidal persistence — pan-stable), **P4-silhouette** (sprite-
outline mask — shadows take the tree shape). Deviation D-1 resolved. The `shadow-cold` RT alloc landed in
P4-core._
