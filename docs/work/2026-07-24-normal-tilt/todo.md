# Todo — normal-tilt

Target: pitch standing-object normals **25° down (= 90° − WORLD_TILT)** into the ground plane
(rigid rotation about the E–W axis). See [README](README.md) / [forks F1](forks.md).
P0/P1/P2/P3 + the split-tooling fix delivered → [completed.md](completed.md).

- [x] P4a — Re-bake conifer. `art remaster biome-thing/default/conifer` runs clean end-to-end; all 9
      variants re-baked with the 25° pitch (log: "pitching object normals 25° down into the ground plane").
      Normal spot-check: flat `#8080FF` bg, E/W relief intact (nx untouched), green carries the pitch.
- [ ] P4b — Verify relief **in-browser** (user/runtime-gated): a light toward the viewer/south brightens
      the front; behind/north darkens it gently — confirm it reads right vs the old ~84° over-pitch. Then
      re-bake the rest of the standing-object corpus (`art remaster biome-thing/default` once P6 lands, or
      per-kind).
- [ ] P6 — Category-level split for new-style sheets: `cmd_split` handles an explicit `sprite.*.png` and a
      kind dir holding them, but a category/subcategory recursion (`split biome-thing/default`) still routes
      through the legacy `_remaster_dirpath`→`_remaster_id` (old-stem) path. Extend the recursion to detect
      per-kind `sprite.*.png` when other new-layout kinds appear.
- [ ] P5 — (deferred) Marigold view-space normal rotation (`bin/art:2341`) — different transform, wire later.

## Cruft noticed (conifer) — not touched
Each `conifer/<v>/` still holds legacy nested leaves from the old `_remaster_id`
(`diffuse.e.0/1/diffuse.png`, plus stale `normal.e.0.png` / `albedo.e.0.png` derived off the pre-split
diffuse). `_find_diffuse` ignores the nested dirs; the flat maps get overwritten on the next `maps` run.
Left in place (didn't create them; deletion is the user's call).
