# Forks — z positioning

_Decision points, options, which we chose and why._

## F1 — The new quantity is called `elevation`, not `z` {#f1}

`z` is already taken in this code, and taken for something almost opposite:

- `slotZ()` returns a **painter's key** — `PAWN_Z_BASE + box.zRow + facingDepth(depth)·SLOT_DEPTH_Z + i·SLOT_ORDER_Z`
- `MoverPart.depth` is *"draw-order offset along the view's depth axis"*
- `prim.zIndex` orders sprites
- `zdepth-world` is the composite carrying the painter's key

None of those is height. `MoverLayer`'s own comment even notes the row "no slot z touches".

- (a) Call it `z`, matching `prim_data.unit.z`.
- **(b) Call it `elevation` everywhere except the record lane it lands in.**

**Chosen: (b).** Adding a second `z` meaning *height* beside four `z`s meaning *order* is how someone
later reads `slotZ` as elevation and spends a day on it. The record field stays `unit.z` because
`VARIABLES.md` owns that name and changing a shipped layout for a naming preference is not worth it —
but everything upstream says `elevation`, and P4 renames the draw-order ones so the collision closes
from both sides.

## F2 — The projection lives in ONE file, in TS and GLSL {#f2}

Elevation must be applied by **two** consumers: the draw path (to place the sprite) and the shadow
path (to place the card). If they disagree, the sprite and its shadow disagree about where the object
is — which is precisely the bug this stream exists to remove, reintroduced one level down.

- (a) Each consumer applies the tilt itself.
- **(b) One `elevationOffset()`, TS and GLSL in the same file, with a dev check that they agree.**

**Chosen: (b)**, exactly as `lightReach.ts` does for `reachFromIntensity` — that pattern has already
paid for itself, and the failure it prevents is silent and asymmetric.

**Derived from `worldTiltDeg`, never a literal.** `__tilt()` re-tilts the whole world coherently
today; an elevation constant baked at a different angle would quietly break that the first time
someone turns the dial.

## F3 — The caster card spans `[z, z + H]`, and this is the load-bearing half {#f3}

`occludes()` models a card rising from the ground: `Lz·(1−t) ≤ H`. A head at height z occupies
`[z, z + H_head]`, not `[0, H_head]`.

**Chosen: span from z.** Without it the head casts *as though it sat on the floor* — a shadow of the
right shape in roughly the right place for entirely the wrong reason, which is the hardest kind of
wrong to notice and the easiest to "fix" by tuning something unrelated.

This is what makes the change **correct** rather than merely *looking* right, and it is why P1 comes
before P3: the geometry has to be true before the head is moved onto it.

## F4 — Head and body will share a base row; decide the tie deliberately {#f4}

Today the head has its own y, so it has its own base row and the painter's key orders it. Once it
shares the body's position, `0x80 | baseRow` is **equal** for both and the compare falls through to
*"equal rows keep warm-over-cold"* — i.e. draw order.

- (a) Leave it to draw order; the carried-piece sequence already puts the head after the body.
- (b) Add elevation to the painter's key as a tiebreak.
- **(c) Keep (a), but WRITE THE RULE DOWN and assert it.**

**Chosen: (c).** (a) is almost certainly correct and costs nothing — the graph already orders carried
pieces — but it is currently true *by accident of iteration order*, and an accident that is never
stated is an accident that gets optimised away. (b) spends bits in a key that is working, to solve a
problem that has not appeared.

**If a case ever shows a head behind its body, that is the signal for (b)** — and P4 records what to
look for so the diagnosis is one glance rather than a hunt.

## F5 — `offset.z` REPLACES `offset.y` on the head; it does not join it {#f5}

- (a) Keep `offset.y` and add `offset.z`, so content can use either.
- **(b) Replace it. The head authors a height and no y.**

**Chosen: (b).** Two ways to move a part vertically is two sources of truth for one fact, and the
whole point of the change is that **y-offset lies to the shadow system**. Leaving it available means
someone reaches for it when the drawing looks off, and the misaligned shadow comes straight back —
with the extra confusion that some parts are correct and others are not.

`offset.y` stays for genuinely horizontal-plane placement (a part that really is further north on the
ground). It is elevation that must not be expressible as y.

## F6 — The coordinate model, settled by the user (2026-08-01) {#f6}

Worked out across four diagrams. **This supersedes my reading in [F2](#f2) and answers
[I4](issues.md#i4).**

### What each field means

`unit.x` / `unit.y` are **screen space** and stay the **drawn** position — the meaning every existing
consumer already assumes (the bake, `zdepth`, presence-by-tile, hit-testing). `unit.z` carries the
depth component that makes the world position recoverable.

For a head carried on a body:

```
Head.unit.y = Body.unit.y - Body.height          // screen-north by the elevation
Head.unit.z = tan(world_angle) * Body.height     // the depth component
Body.unit.z = 0                                  // standing on the ground
```

### Why this is right and my proposal was not

I argued for storing the **ground** position and computing the drawn one. That would have redefined
`unit.x/y` under every existing reader — a far larger change — and it was unnecessary, because **no
information is lost either way**:

```
elevation = unit.z / tan(world_angle)
ground_y  = unit.y + elevation
```

Body and head reconstruct to the **same ground point** even though their `unit.y` differs. That is the
alignment this stream exists to produce, and it costs the draw path nothing — the hot per-sprite path
uses `unit.x/y` as-is.

### The DSL authors `elevation`, not `unit.z`

`unit.z` is a **derived, tilt-dependent** quantity; `elevation` is the authored fact ("this sits
`Body.height` off the ground"). Content authors elevation, and the writer decomposes it. That keeps
`__tilt()` coherent: a tilt change re-derives `unit.z` rather than silently invalidating a baked one.

### There is NO draw-side constant

[I4](issues.md#i4) hunted for a projection factor to apply when drawing. There isn't one: the
screen-north shift **is** the elevation, 1:1. The tilt enters only in `unit.z`'s decomposition and in
the shadow's screen↔world transform.

## F7 — Three coordinate systems, and `prim_data` holds GAME (user, 2026-08-01) {#f7}

**Supersedes [F6](#f6).** F6 had `unit.*` doing double duty — the stored drawn position *and* the
thing game logic reasons about. The user split them:

| system | is | owned by |
|---|---|---|
| **game** `unit.x/y/z` | the tile the billboard is physically in, plus **elevation** along the world_angle ray | the CPU, and `prim_data` |
| **screen** `screen.x/y/z` | `unit.x`, `unit.y − unit.z`, `tan(angle)·unit.z` | the draw path |
| **world** `world.x/y/z` | true 3D | derived for shadow maths |

Game is the source; screen and world are both projections of it.

**`presence` and the caster buckets key on GAME coordinates.** That is the decision that makes head and
body align *by construction*: they occupy the same tile because they are in the same place, and no
reconstruction is needed at read time.

### Why this is also the cheaper arrangement

The conversion has to live somewhere. On the shadow side it would be paid **per caster test, per
light, per texel** — the hot loop. On the draw side it is one subtraction **per prim per frame**.

And the draw side barely pays even that: **`prim_data` is read only by the lighting and shadow
shaders.** `SquareCache` and `mrtBakeShader` contain no `uData` — the bake draws from the `Primitive`
objects, on a path that never touches the record. So the record can hold game coordinates without the
renderer noticing.

### What the rename buys

A CPU-side `unit.y` that silently differs from the shader's `unit.y` is a bug generator. Naming the
two systems apart makes the difference impossible to overlook — which matters more here than usual,
since this stream exists because a drawn offset was being mistaken for a position.

### Still to pin

- **Which space is `fine.z` in?** `fine.x/y` refine the drawn sub-unit position. If `fine.z` refines
  *elevation* it is game-space; if it refines `screen.z` it is screen-space and the CPU must split
  `tan(angle)·elevation` into `u8 + u4`. Getting this wrong puts the two out of step by a `tan` factor.
- **What space is the lighting texel grid in?** The blit samples the lightmap by `vWorld = aPosition`,
  which is the drawn quad — i.e. screen. If the records are game-space, the receiver coverage test and
  `occludesAt` are exactly where the two meet, and that is where the conversion belongs.
