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


## 2026-08-01 · P0 — height is observable, and it discriminates

### The shadow path, re-read before touching it ([I1](issues.md#i1))

The plan's first item exists because two diagnoses in this stream's predecessor were given from
memory of deleted code. The live text of `occludesAt` (`records.ts`, `OCCLUSION_GLSL`):

```glsl
float h = mix(Lz, targetH, t);          // t = (C.y - L.y) / (P.y - L.y)
if (h > hTop || h < hBot) return false; // hTop = subH, hBot = 0.0  -- the card, bottom-aligned
```

`h` is the light ray's height at the caster's row, tested against a card spanning `[0, subH]`. The
machinery is real and the heights are interpolated — **I1's "heights are not populated" was correctly
withdrawn**, and [F3](forks.md#f3)'s `[z, z+H]` change is a change to `hBot`/`hTop`, not a repair.

### It discriminates — measured, not argued

Instrument: `__gather()`, which reports live shadow-texel counts and re-checks the walk against an
exhaustive brute-force reference. Every emitter's `unitZ` swept, everything else held:

| `Lz` | shadow texels | corridor walk | walk vs brute |
|---|---|---|---|
| 0 | 1016 | 503 | **0 differing** |
| 20 | **0** | 0 | **0 differing** |
| 40 | 1677 | 835 | **0 differing** |
| 80 | 1016 | 503 | **0 differing** |
| 160 | 446 | 222 | **0 differing** |
| 255 | 213 | 105 | **0 differing** |

Above the caster tops the count roughly halves per doubling of `Lz` — the shape `d·H/(Lz−H)` predicts.
**`Lz = 20 → 0 shadows` is correct behaviour, not a defect**: `content/visual/things.rd:188` documents
that a light below a caster's top makes `k = Lz/(Lz−Zt)` go negative and the caster registers nothing.
That is a *registration* gate upstream of `occludesAt`, and this sweep is the first thing to exercise it.

**Conclusion, which is what the item was for: the machinery works. The gap is the metric**
([I9](issues.md#i9)) — exactly as the plan anticipated, so P1/P2 proceed as written.

### Two real defects found and fixed on the way

**`placeLights` wrote no `unitZ` at all.** Every debug-placed light sat at `Lz = 0`, where the solve
degenerates: `h = mix(0, 0, t)` is identically 0 over a ground receiver, so `h > hTop || h < hBot`
can never reject and every caster occludes at every distance. **Every visual check in
`2026-07-31-lighting-rework` ran through this hook** — which is why its shadows were *"not remotely
correct"* while the corpus's own torches, which do author 2.5 tiles, were fine. Default is now the
corpus's documented 40 units, so the debug path and the content path agree by construction.

**`stepOrbit` dropped it again.** `writePrim` writes a *whole* record, so the orbit step's omission
put every moving light back on the floor each frame. The height now rides on `liveLights` and survives
the re-write. This is the "moving lights" path specifically, so its measurements were degenerate too.

### The elevation probe ([`__elev`](../../../client/webgl/src/game/viewport/Viewport.ts))

Answers "how high does the code think this is, and where does it draw it" in one call, because the
three numbers live in three places — elevation in the record, card extent in the definition, projection
in `worldTilt`. Verified against a known fixture: a caster declared 24 units tall reports `cardH: 24`,
and lights placed at 40 report `elevation: 40, worldZ: 85.78` (= 40 × 2.1445).

Its first version reported `cardH: 1` for a 24-unit card — it indexed the definition as
`block + rotation` where `writeDefinition` uses `block * ROTATIONS_PER_DEF + rotation`. Caught by
checking the probe against a fixture with a known answer, which is the only reason it was caught.

### `__caster` — a deterministic caster ([I14](issues.md#i14))

Places a solid `w × h` box through the real `allocPrim`/`writePrim`/`writePresence` calls. The
silhouette is skipped deliberately: `silhouetteHit` returns its `offPage` argument for a definition
past the two bound atlas pages and `occludesAt` passes `true` there — the documented *"conservative
answer (casters: the solid box)"* — so parking the def on page 3 yields an exact rectangle. A
disagreement is then geometry, never art.

### Corrected mid-task

I reported "nothing casts and nothing receives" from a record dump taken **before the scene finished
syncing**, and nearly filed it. With the scene loaded there are **525 casters and 1033 shadow texels**,
and `firstFrameNonZeroTexels: 0` — which I also nearly read as current state — is a startup statistic.
The lesson is the stream's own rule: this codebase populates asynchronously, so a count of zero means
"not yet" until proven otherwise.
