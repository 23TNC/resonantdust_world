# Completed — normal-tilt

- **P0 — shared world-tilt constant in `bin/art`.** `WORLD_TILT_DEG=65` + `NORMAL_PITCH_DEG=$((90 −
  WORLD_TILT_DEG))` = 25, defined by the `GRID_MAPS` config block, commented to stay in lock-step with
  the client's canonical `shadowGather.ts` `WORLD_TILT_DEG` (no shared bash⇄TS source).
- **P1 — `_normal_tilt` is now a rigid rotation.** Replaced the additive bias+Z-squash with a pitch of
  `NORMAL_PITCH_DEG`° down about the E–W (screen-x) axis: `ny' = ny·cos − nz·sin`, `nz' = ny·sin + nz·cos`
  (nx fixed, renormalized). `cos`/`sin` computed via awk. `_tilt_kind_normals` + both call sites
  (`cmd_normal`, `cmd_maps`) pass `NORMAL_PITCH_DEG`. Verified: flat `#8080FF` → `(128,74,243)` =
  `(0,−0.423,0.906)` — an exact 25° down-pitch (was ~84° under the old 1.8/0.18 bias).
- **P2 — per-facing sign: NOT needed (resolved by analysis).** The pitch seats the *card* into the ground
  plane — a property of the card's world orientation, not the depicted content. Every billboard faces the
  viewer, so every facing's *visible* surface faces the viewer (south): S-facing shows the front, N-facing
  shows the rear (which faces south toward us), E/W show the south-facing flank. So the single south-ward
  pitch is correct for all four; E/W relief is preserved anyway (nx untouched by the E–W-axis rotation).
  Light-dir check: N-facing wolf under a north light backlights its (south-facing) rear → darkens ✓.
  ⇒ uniform `NORMAL_PITCH_DEG` across facings, no facing gate in `_normal_tilt`.
- **Split tooling — new folder-per-variant source sheets (enables the P4 re-bake).** `cmd_split` now
  recognizes a `sprite.<dir>.<layer>.png` at the KIND level as a multi-variant source sheet (map `sprite`
  = raw multi-variant art; ID/kind + variant come from the directory position — the sheet sits one level
  above the numbered variant leaves). New `_split_variant_sheet` keys + edge-bleeds it and writes each
  reading-order blob to the go-forward FLAT leaf `<kind>/<K>/diffuse.<dir>.<layer>.png` (what
  `_find_diffuse`/`maps` read), pruning stale higher-index variants for that facing. The legacy
  `_remaster_id` path (old `<id>.<rot>.<layer>/<variant>/<map>.png` nested leaf) is untouched. Verified:
  `art split biome-thing/default/conifer/sprite.e.0.png` → 9 variants into `conifer/{0..8}/diffuse.e.0.png`,
  ordering correct (0=top-left small, 6=bottom-left), magenta keyed out. `bash -n` clean.
- **Remaster no longer dies after the split (cmd_outline collision).** Two `cmd_outline` defs existed: the
  image-**border** drawer (@511) and the shadow-**silhouette**/meta.json generator (@3173, which used
  `exec`). Bash kept the second, so remaster's border call hit the silhouette fn and its `exec python3`
  **replaced the shell**, truncating remaster right after the split (before maps/layers/pack). The
  silhouette is already generated inside `maps` (calls `outline.py` directly), so the collision was pure
  regression from `34e7fd0`. Fix: renamed the border fn → `cmd_border` (+ `border)` dispatch), pointed
  remaster at it, removed the `exec` (+ `exit 1`→`return 1`) so `cmd_outline` can't nuke a caller, deduped
  the double `outline)` dispatch. Verified: `art remaster biome-thing/default/conifer` now runs ALL steps
  — split → border → normal(laigter) → albedo+residual(marigold) → strength/AO → **25° pitch** → surface →
  silhouette(meta.json, earcut OK) → channel-pack(layers). All 9 variants carry the full map set.
- **P3 — retired `tilt_amount`/`tilt_z`.** Dropped from `cmd_normal`/`cmd_maps`/`cmd_remaster` defaults +
  positional arrays + the remaster `tilt_args` forwarding; `--tilt`/`--no-tilt` category gate kept. Help
  text + usage lines updated. `grep tilt_amount|tilt_z` is clean. `bash -n` passes; docs-check green.
