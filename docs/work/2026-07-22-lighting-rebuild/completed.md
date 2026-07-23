# Completed — lighting rebuild

_Done **and** verified on `/overlayRT shadow-cold`. Items move here from [`todo.md`](todo.md).
Append-only history; authoritative for what's done._

---

## P0 · Strip the mess — 2026-07-22 ✓ (archive deferred)

- Removed all debug scaffolding from `shadowGather.ts`: the reject-bit classification overlay (back
  to plain coverage tint), the `__setLight` / `__only` / `__kept` hooks + `debugOnly` field + the
  `tick()` caster filter. The forced-`0` overrides fell out with the pure-quad rewrite (penumbra,
  base-pad `f`, `cover()` gone); `SHADOW_LIFT = 0` remains as a real dial.
- Corridor march + `u/v` inversion already replaced by the geometric quad + brute-force walk.
- **Deferred:** archiving `2026-07-22-shadow-corridor` out of the repo — held to the end of the
  rebuild so the cross-links (README/issues reference it) don't break mid-stream. Still in
  [`todo.md`](todo.md).

## P1 · Data textures — 2026-07-22 ✓

- Light data (position, colour, reach), per-tile **presence** (≤8 lights), per-tile **caster
  buckets** (≤8 casters) all live (carried from the prior gather shell). One light seeded at tile
  **(54,21)**, **z = 40 units**, reach 12 tiles. Gizmo draws the light dot + reach ring.

## P2 · Geometric quad shadows (brute force, units) — 2026-07-22 ✓

- Occlusion is **point-in-convex-quad** (`shadowCover`, four `cross2` edge signs) — texture-agnostic,
  all in **units**. Caster card = **2 tiles / 32 units** tall, tilted 65° north + elevated; light
  `z = 40 units`. Per texel: presence → walk every tile in reach → test each caster's projected quad;
  occluded → write the light's colour. Overlay displays by light colour.
- **Zoom-independence FIXED + verified.** `ColdShadowData.definitionFor` no longer gates on the
  surface LOD resolving (pure quad needs only prim W/H) — the zoom-dependent dropout
  ([`issues.md#zoom`](issues.md)) is gone. Verified quads render identically at zoom **0.5, 1, 2**.
- Still brute-force (walk all in-reach tiles); the corridor returns in P6 validated against this.
