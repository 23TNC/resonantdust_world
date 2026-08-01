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

### Superseded — `screen.z` IS the tiebreak (user, 2026-08-01)

> "screen.z is perpendicular to the screen. screen.z would make a decent z-ordering metric, so long
> as we calculate screen.z against a common reference point."

(c) was a fallback chosen because there was no *principled* tiebreak available. There is one, and this
stream produces it anyway: a depth perpendicular to the screen is exactly what a painter's key wants,
and it orders head against body **because the head is nearer the viewer**, not because of the order
the graph happened to walk its pieces.

The condition is the one the user attached: **a common reference point**. A depth measured from each
prim's own origin is not comparable between prims; measured from one scene-wide plane it is. That is
the acceptance for the item, and it is what makes this different from (b) — (b) proposed bolting
elevation onto the existing key, whereas `screen.z` *replaces* the ad-hoc key with the real quantity.

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

### `fine.z` is game-space (user, 2026-08-01)

In units, like `fine.x/y`. The elevation is therefore **`unit.z + fine.z/16`**, and both terms enter
the screen conversion together — `fine.z` is not a separate refinement applied afterwards.

## F8 — Solve in GAME space; convert `P` once per texel, not the casters {#f8}

Follows from [F7](#f7). Once records hold game coordinates, *something* has to bridge them to the
lit point `P`, which comes off the lightmap grid in screen space.

- (a) Convert each caster where it is read.
- **(b) Convert `P` once at the top of the shader; run the whole solve in one space.**

**Chosen: (b).** (a) pays **per caster test, per light, per texel** — the hot loop, and the reason
the walk exists at all. (b) pays **once per texel**, using the receiver record the shader *already*
fetches. Same result, one conversion instead of thousands.

### The target space is WORLD, not game (user, 2026-08-01) — corrects this fork

I first wrote "solve in game space". Wrong target; the cost argument above is unaffected, but the
destination is not game.

> "the head reports unit.x/y/z and that converts into world.x/y/z which are used to determine
> shadows, and those are then used on the screen using screen.x/y"

**Lighting textiles are screen-space** — they are drawn to the screen. So `P` is screen, records are
game, and the solve is world: *both* inputs convert, and they convert to world.

**Why solving in game would have looked fine and been wrong.** game→world is **affine**, and affine
maps preserve segments, collinearity and ratios along a segment — so *occlusion* would have come out
identical. But two things in the same shader are **metric**, and affine maps with unequal axis scales
do not preserve them:

- `d = distance(Puse, Lpos)` against `reach` — **falloff**
- `ldir` — **`N·L`**

Both would have been quietly biased in north-south, in a way that looks like a tuning problem rather
than a coordinate problem. Solving in world costs the same and is right for all three.

### The conversion is smaller than it looks

`P = (vec2(tileX, tileY) + inTile) * UPT` — a **square** grid, `UNITS_PER_TILE = 16` on both axes.
There is no y-foreshortening in this renderer; tiles are drawn square. So game and screen differ by
**elevation and nothing else** — no `cos`, no `tan`, no per-axis scale.

And a billboard card is a **vertical plane at one ground y**: every texel on it shares the card's
game y. So for a card receiver at elevation `E` with screen base `baseY`:

```
Pgame   = vec2(P.x, receiver.unit.y)      // the card's own ground row
targetH = E + (baseY - P.y)               // height above the GROUND, not above the card's base
```

and for the ground receiver, elevation is 0, so `Pgame == P` and nothing changes.

### This is also the `targetH` bug

Today's `targetH = max(0.0, baseY - P.y)` is that expression **with `E` dropped** — correct only while
every receiver stands on the floor. It is the head case, already wrong, on the receiving side.

### It retires my "isotropy gap"

I flagged `targetH` as mixing a screen-space y-difference with authored-unit heights, needing an
`nsInv = 1/cos(angle)` correction. **There is no such gap** — the grid is square, so a screen y
difference already *is* a unit difference. The defect is the missing `E`, not a missing metric.


## F9 — `unit.z` currently means TWO things, and P2 cannot proceed until it means one {#f9}

[I8](issues.md#i8) resolved the coefficient and immediately exposed a deeper problem: the same lane
holds two incompatible quantities.

| written by | value | is |
|---|---|---|
| `RecordSync`, for a **light** | `L.height / SQUARE * UNITS_PER_TILE` | a **world** height — `SquareCache.height` is *"world px above the ground plane"* |
| what P3 will write, for a **head** | the part's elevation | a **drawn** up-screen shift, per the user's `screen.y = unit.y − unit.z` |

These differ by `sin(θ)` — about 10% at 65°. P2 has to convert drawn terms to world, and it cannot,
because it cannot tell from the lane which kind a given `unit.z` is.

- **(a) The lane stores a WORLD height.** Matches what lights already do and how content authors
  ("2.5 tiles up"). Costs: the draw path becomes `screen.y = unit.y − unit.z/sin(θ)`, contradicting
  the user's explicit 1:1 rule.
- **(b) The lane stores a DRAWN shift.** Matches the user's rule exactly and keeps the draw path
  free. Costs: `RecordSync` must divide light heights by `sin(θ)` on the way in — **every light in
  the world gets ~10% higher**, so every shadow shortens.

**Recommend (b)**, because the 1:1 draw rule is the user's stated design and the whole reason the
head's y-shift becomes a rendering consequence rather than an authored lie. The conversion then lives
in exactly one place — the record writer — and everything downstream reads one kind of number.

**Not chosen unilaterally.** (b) changes shipped lighting for every existing torch, and the 40-unit
contract in `content/visual/things.rd` was tuned by hand against the current meaning. That is a
content-visible change, so it is the user's call.

**Blocks P2 only.** P0a/P0/P0b/P1 are all landed and independent of it.
