# Forks — shadow-projection

_Decision points + options + which we chose + why. Resolve before (or as) the phase lands._

---

## F1 · Where the caster's facing (rotation) comes from — 2026-07-21 (open)

The two-regime setup (P5) needs each caster's facing (0=S,1=E,2=N,3=W). A `Primitive` carries `flipX` + `cell`
(the variant) but **not** the rotation — `thingTexture` computed the facing during expansion and kept only
`flipX`. Options:

- **(a) Add a `facing` field to `Primitive`** — `WorldBridge.onColdThings` already knows it (`FACING_BY_ROTATION`
  / `DEFAULT_FACING`); store it on the prim. Cleanest; a small type addition. Cold things are single-facing
  (`DEFAULT_FACING`), movers pick from rotation — both already resolved at add time.
- **(b) Derive from `flipX` + the sprite stem** — brittle (flipX only distinguishes E vs W), can't recover N/S.
- **(c) One regime for now** — cast every caster E/W (side-on) until movers/facings matter. Simplest start;
  wrong for front/back casters.

**Lean:** (a) — the data exists at add time and the field is cheap; it also feeds any future per-facing bake
(the presence map is per-facing, P2).

## F2 · Solid triangles first, or alpha-masked from the start — 2026-07-21 (open)

- **(a) Solid tris first (P3), alpha mask after (P4)** — get the projected fan shape rendering + verified before
  adding UV sampling. Smaller steps, each verifiable; matches the phase split.
- **(b) Alpha-masked from the start** — the "real" look immediately, but couples the projection port to the UV
  path (harder to isolate a bug).

**Lean:** (a). The sandbox itself has a solid-tri mode (its default) precisely because the shape is the thing to
get right first.

## F3 · Geometry batching + how the fan feeds the shadow field — 2026-07-21 (open)

Each (light, caster) is 5 triangles. With N casters × up-to-6 lights that's a lot of small draws. Options:

- **(a) One instanced draw** over all (light, caster) fans — an instance buffer of the 15 projected vertices
  (or the corners + a vertex-shader that builds the fan), one draw per frame. Best throughput; more setup.
- **(b) A batched dynamic vertex buffer** — CPU-build all fans into one buffer, one draw. Simpler than
  instancing, still one draw; rebuilt per frame (the cast is per-frame anyway, like today).
- **(c) Per-caster draws** — simplest, slowest; fine for the first port with few casters in a light's reach.

Relatedly, **where the fan lands**: this stream draws into the SAME screen-space field the current cast writes
(per-light colour, default-on overlay). Folding it into the persistent world-cold **bitfield** + round-robin is
the [`shadows`](../shadows/README.md) stream — the fan is its `shadow-hot` generator, so keep the raster target
swappable.

**Lean:** (b) to land the port (one dynamic buffer, CPU-built like the sandbox), revisit (a) if the per-frame
rebuild costs; the analytic per-pixel loop it replaces was the real cost, so even (c) likely wins.

## F4 · Where `θ` (ground angle) + the depth bake live — 2026-07-21 (open)

- **`θ`**: the sandbox uses `θ≈65°` (E/W) / `115°` (N/S) — an **art-style constant** (the oblique 3/4 view
  angle), likely one shared value (+ the N/S roll), not per-def. Start with a constant matching the art-style
  spec; expose per-def/DSL only if art needs it.
- **Depth (`dA/dB`)**: **auto from the presence map** (P2) is the default rule; the DSL override is deferred
  (`design/shadows.md`: "computed from the sprite, override in the DSL when art needs it"). The presence bake is
  a per-sprite-facing step — do it once on texture load (W4h gives the silhouette), cache per def+facing, not
  per frame.

**Lean:** constant `θ` (art-style), auto depth from presence, no authoring surface this stream.
