# Todo — normal-tilt

Target: pitch standing-object normals **25° down (= 90° − WORLD_TILT)** into the ground plane
(rigid rotation about the E–W axis). See [README](README.md) / [forks F1](forks.md).

- [ ] P0 — Shared `WORLD_TILT_DEG` source for the bake (mirror `shadowGather.ts:52`); `bin/art` derives
      `θ = 90 − WORLD_TILT_DEG` (= 25°) from it, so nothing is duplicated or eyeballed.
- [ ] P1 — Replace `_normal_tilt`'s additive bias with the rigid rotation:
      `ny' = ny·cosθ − nz·sinθ`, `nz' = ny·sinθ + nz·cosθ` (nx unchanged; renormalize). Keep the runtime
      `uNormalYSign(-1)` convention.
- [ ] P2 — Per-facing sign table (F1 open detail): confirm E/W/S/N pitch directions before corpus re-bake.
- [ ] P3 — Retire `tilt_amount`/`tilt_z` knobs + `--tilt_amount`/`--tilt_z` flags (keep `--tilt`/`--no-tilt`
      category gate).
- [ ] P4 — Re-bake the standing-object corpus; verify relief direction in-browser (a light toward the
      viewer/south brightens the front; a light behind/north darkens it, gently).
- [ ] P5 — (deferred) Marigold view-space normal rotation (`bin/art:2341`) — different transform, wire later.
