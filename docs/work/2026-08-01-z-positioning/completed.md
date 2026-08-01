# Completed — z positioning

_The verification log: dated entries saying what landed and **how it was checked**. Append-only._

## 2026-08-01 · P0a — the tilt exists again, and the coefficient is pinned

**`client/webgl/src/game/viewport/worldTilt.ts`** — new, and the only place the angle appears.

`WORLD_TILT_DEG = 65`, **measured from the ground plane** (0° looks along the ground, 90° straight
down). The axis is named because the same number read from the vertical is a nearly-horizontal
camera, which would invert every height in the scene — that is what the item's acceptance was
guarding against.

### The split ([I8](issues.md#i8)) — `world.y` gets nothing, `world.z` gets `tan(θ)`

Derived rather than picked, from two facts already settled:

1. **The ground planes coincide.** `P = (vec2(tileX, tileY) + inTile) * UPT` is a square grid,
   16 units on both axes, and the art draws every tile square — the 3/4 view lives in the sprites,
   not the projection. So a drawn tile of northing IS a world tile of northing, and `world.y = unit.y`
   with no term from `unit.z`.
2. **The stored elevation is the up-screen SHIFT**, not a height and not the ray's length. Forced by
   the user's own placement rule ([F6](forks.md#f6)): *"the screen-north shift IS the elevation, 1:1"*.
   A northing component of `E` on a ray rising at θ means a true height of **`E·tan(θ)`**.

`TILT_TAN ≈ 2.144507`. It is the **one** coefficient, and it is metric — it scales `world.z`, which
feeds the caster card's extent, the falloff distance and `N·L`. A wrong value there changes how
bright things are, not just where a shadow lands.

### Verified

- `npx tsc --noEmit` clean.
- `elevation → height → elevation` round-trips across the whole `u8` range, worst error
  **2.8e-14** — floating-point noise, not a modelling error.
- TS and GLSL cannot disagree: `WORLD_TILT_GLSL` interpolates `TILT_TAN` from the TS constant, the
  same construction `LIGHT_LANES_GLSL` uses next door ([F2](forks.md#f2)).

### Left honest

`screenDepth()` and `worldHeightForElevation()` currently share the `tan(θ)` coefficient. Whether
screen-perpendicular depth really wants the same factor as true height is the residue of
[I8](issues.md#i8) — but **z-ordering is invariant under any positive scale**, so if it is wrong the
ordering it produces is unchanged and only `world.z` would move. Documented at the function.
