# Todo — normal-tilt

Target: pitch standing-object normals **25° down (= 90° − WORLD_TILT)** into the ground plane
(rigid rotation about the E–W axis). See [README](README.md) / [forks F1](forks.md).
P0/P1/P2/P3 delivered → [completed.md](completed.md).

- [ ] P4 — Re-bake the standing-object corpus (`bin/art remaster <kind>` / `art maps`) and verify relief
      direction **in-browser** (a light toward the viewer/south brightens the front; behind/north darkens
      it, gently — no longer the ~84° over-pitch). User/runtime-gated: needs the laigter pipeline + a
      browser look. Shipped normals stay the old ~84° pitch until this runs (`/textures` is gitignored).
- [ ] P5 — (deferred) Marigold view-space normal rotation (`bin/art:2341`) — different transform, wire later.
