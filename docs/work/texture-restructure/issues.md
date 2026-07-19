# Issues — texture-restructure

_Problems hit + how they were resolved. Chronological. Convention:
[`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

- **2026-07-19** · **The plan was built on an out-of-date linked/autotile model.** As opened, the
  stream treated the on-disk `linked/<form>.<material>/1.l.0/1..16/` per-cell folders as the current
  reality and asked (F3) whether the `1..16` were auto-tile pieces or art variations to remap. **They
  are the superseded per-cell split.** The live edge + client already use a **held-whole autotile
  atlas**: one master texture with a `cols×rows` cell grid + an `atlas.json` (`grid`,`pad`) sidecar,
  client sampling a cell by UV ([`tex_manifest.rs`](../../../server/edge/src/tex_manifest.rs),
  [`textureManifest.ts`](../../../client/pixijs/src/textures/textureManifest.ts),
  [`SquareCache.ts`](../../../client/pixijs/src/game/viewport/SquareCache.ts)). The disk was
  **half-migrated** (wall.smooth = atlas, wall.blueprint = 16-split, fence/rock = empty), which was
  the tell.
  **Resolved (user 2026-07-19 → held-whole atlas):** conformed the plan + design to the atlas model —
  F3 closed (no per-variant remap; one atlas per form + `atlas.json`, per-cell folders dropped),
  README scope + design §7 note the atlas sidecar, P3 **re-masters** the still-split/empty kinds
  rather than uniformly renaming. Found by the verify-before-you-work pass, using the docs-authority
  discipline. (This is why the audit + verify step exists — the plan read plausibly but was stale.)