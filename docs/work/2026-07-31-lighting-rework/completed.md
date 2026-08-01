# Completed — lighting + shader rework

_The verification log: dated entries saying what landed and **how it was checked**. Append-only._

_Nothing yet — the stream is planned, not started._

## 2026-07-31 · P0 — ground rules, formats, and the instrument

### The gate is met

`shadowGather.ts` and `coldShadowData.ts` are gone, the strip is `done` 20/20, the page loads unlit at
1 draw. This stream builds from that floor.

### `RGB10_A2` proven on the metal, not assumed

Added as a `TexFormat` (`gl.RGB10_A2` / `UNSIGNED_INT_2_10_10_10_REV`) plus a boot assert in
`renderer.ts`, in the style the file already uses for the float extensions — and for the reason it
gives: **this class of failure is silent.** An unrenderable attachment makes the FBO incomplete, and a
lighting pass drawing into an incomplete FBO writes nothing while raising no error, which on screen is
indistinguishable from "the lights are off".

`assertRenderable()` allocates a 1×1 texture, attaches it, and asks the driver rather than trusting
the spec's renderable-format table.

Then verified the part the assert *cannot* check — that it actually blends:

| | |
|---|---|
| FBO complete | **true** |
| two `ONE,ONE` draws of 0.25, read back | **128/255 = 0.502** |
| `getError()` | 0 |

An integer format would have read back 0.25 (ES 3.0 §15.1.4 skips blending for integer formats — the
trap the existing comment documents). This is fixed-point, so it accumulates, and
[F2](forks.md#f2)'s premise holds on this machine.

### The instrument: `__framecost()`

[`frameCost.ts`](../../../client/webgl/src/game/viewport/frameCost.ts) — hand-driven ticks, wall clock
with `gl.finish()` at both ends, median of 5, plus an exact draw count. Its header records **why not
timer queries**, with the measured numbers from [strip I7](../2026-07-31-lighting-strip/issues.md#i7):
every faster drain returns plausible partial data rather than erroring.

Reproduces the floor across two consecutive runs — **0.035 and 0.028 ms median, 1 draw**, each inside
the other's spread.

> **The plan's acceptance said 0.045 ms and that number was stale** — it is P1's figure, and P2/P4
> took the floor to 0.028 by deleting `moverDirty`'s per-frame record rewrites and the cursor light.
> Recorded rather than quietly re-baselined; the instrument agreeing with the *current* floor is the
> check that matters.

### `reachFromIntensity` — one definition, two languages, one file

[`lightReach.ts`](../../../client/webgl/src/game/viewport/lightReach.ts) holds the TS function **and**
the GLSL, deliberately in the same file: [F6](forks.md#f6)'s binding constraint is that the CPU
building each tile's light set and the GPU bounding the walk cannot disagree, and separate files is
how that starts.

Falloff `L(d) = I / (1 + (d/d0)²)` solved at `L = ε`, with `ε = 1/255` (under one 8-bit level) and
`d0 = 1 tile`:

```
d = d0 · sqrt(I/ε − 1)
```

**Full intensity lands on exactly 16 tiles** — chosen so the worst case content can author is the same
worst case the stream's headline prices.

`__reachcheck()` runs the GLSL against the TS at **all 1024** `u10` values:

| | |
|---|---|
| worst absolute difference | **2.3 × 10⁻⁵ units** (at intensity 575) |
| agree (< 1e-3) | **true** |
| max reach | **16 tiles** |

That residual is float32 rounding between the two evaluations, not a difference in logic.

Also added `UNITS_PER_TILE = 16` to `squareMath.ts` — a world invariant that was a bare literal in a
dozen derivations.

**Verified:** page loads clean at the fixture, `getError()` 0, frame still 1 draw at 0.030 ms.

## 2026-07-31 · P1 — the record layer

[`records.ts`](../../../client/webgl/src/game/viewport/records.ts) — `prim_data` + `definition_data`,
their mirrors, the allocators, and the lane assertions. Two separate `RGBA32UI` textures (1 MiB +
256 KiB) rather than banded regions of one: with no `set` nibble there is nothing to gain from packing
them together, and separate textures mean **an index is just an index** — which is the whole point of
the flat model.

### Every acceptance, checked against the live scene

`__records()` runs on the real renderer, not a fixture:

| acceptance | result |
|---|---|
| allocation never returns the sentinel ([F8](forks.md#f8)) | **true** over 64 allocations |
| `base + rotation`, 4-rotation billboard | frames `[10, 11, 12, 13]` — one add, no special case |
| `base + rotation`, 16-cell linked tile | rotations 0/7/15 → `[200, 207, 215]` |
| `frame.span` un-biases | **true** (stored 3, reads 4) |
| rotation clamp ([F7](forks.md#f7)) | asked 9, allocated 4, **got 3** |
| lane too wide throws, not truncates | intensity > `u10` ✓, unit > `u16` ✓, span 0 ✓, valid write succeeds ✓ |
| round-trip a real prim | **exact** — 1764/1032 units, rotation 2, intensity 512 |

**One expectation was mine and wrong.** The first run reported `rotationOver4Bits: false` — writing
rotation 99 did not throw. That is correct *by design*: `writePrim` clamps through `clampRotation`
before the lane check, which is what [F7](forks.md#f7) specifies (validate on write, so the GPU pays
nothing). The test asserted a throw where the contract promises a clamp. Corrected to assert the
clamp — the code was right and the check was measuring the wrong contract.

### The atlas packer already existed ([I8](issues.md#i8))

Verified by sampling rather than rebuilt: four equal quadrants of a 256×256 frame,
`fullframe.span == 2 × frame.span`. **And it disagrees with the design on BL/BR** — live is
`surface [0,1]`, `layers [1,1]`; the doc has them swapped, on the four lines prefixed *"I believe that
puts…"*. The mapping is live, so the doc is what needs the correction. Raised, not silently edited.

### `__buildrecords()` ([I9](issues.md#i9))

P1's last item asked the G-buffer bake to draw from records; the design keeps the bake (art → channels)
and the records (what shaders read) apart, so that was logged as a plan error and its *intent* built
instead: mint a record for every live prim from the resolver, then cross-check each decoded
`frame.x/y` back against the resolver's own frame.

**455 prims, 2 definitions, 0 frame mismatches.** A re-route would prove the records are sufficient to
draw from; this proves they are *accurate*, which is what a shader holding a bare `u16` actually needs.

**Verified:** page loads clean, `getError()` 0, frame still **1 draw at 0.032 ms** — the record layer
costs the render path nothing, because nothing renders from it yet.
