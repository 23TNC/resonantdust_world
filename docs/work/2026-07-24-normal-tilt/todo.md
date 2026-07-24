# Todo — normal-tilt

Target: pitch standing-object normals **25° down (= 90° − WORLD_TILT)** into the ground plane
(rigid rotation about the E–W axis). See [README](README.md) / [forks F1](forks.md).
P0/P1/P3 delivered → [completed.md](completed.md).

- [ ] P2 — Per-facing sign table: confirm E/W/S/N pitch directions before the corpus re-bake. Current bake
      pitches every facing the same way (green → negative); N-facing sprites may want the opposite sign,
      E/W may want none. Decide with the user, then gate `_normal_tilt` by facing if needed.
- [ ] P4 — Re-bake the standing-object corpus (`bin/art remaster <kind>` / `art maps`); verify relief
      direction in-browser (a light toward the viewer/south brightens the front; behind/north darkens it,
      gently — no longer the ~84° over-pitch).
- [ ] P5 — (deferred) Marigold view-space normal rotation (`bin/art:2341`) — different transform, wire later.
