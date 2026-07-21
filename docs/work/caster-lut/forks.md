# Forks — caster-lut

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · Texture sizing — uniform 1024×12 vs per-texture heights

- **Uniform `1024×12` for all three (chosen default).** One allocation shape, one code path, 192 KB each.
  Prim + light get 4,096 def slots; LUT gets 49,152 entries.
- **Per-texture heights.** `1024` width is the fixed safe dimension; height is what actually needs tuning:
  the **prim** texture is the capacity-critical one (4,096 < the 7,440 tiles resident at zoom-out) and would
  grow first — add 3-row bands → `1024×24` = 8,192 casters. The **light** texture is over-provisioned at
  4,096 for 5 lights and could shrink to `1024×3` (1,024 lights) to reclaim 192 KB.

**PICK:** start uniform `1024×12` (simplest, per the design); grow the prim height in 3-row bands when a
dense scene approaches 4,096; only shrink the light texture if the 192 KB matters. _(pending execution)_

## F2 · LUT structure — per-light runs vs per-tile/cluster bins

- **Per-light contiguous runs (chosen).** Light stores `(start, count)`; its casters are a run in the LUT.
  Simple and correct. Cost: a caster in range of M lights appears M× (LUT size = `Σ` per-light counts, not
  unique casters), so heavy light overlap inflates the LUT.
- **Per-tile / cluster bins.** Bin casters by screen tile/cluster (clustered-shading style); each light
  reads the bins it covers. Dedups overlap → smaller LUT under heavy overlap, but more machinery (bin
  assignment, per-light→bin mapping).

**PICK:** per-light runs — simplest and the texture formats don't change if we later swap the LUT-build
strategy. Revisit only if `Σ count` pressures 49,152 ([I-4](issues.md#i-4)). _(pending execution)_

## F3 · What consumes the textures — interim JS cast vs shader cast

- **Shader cast (the goal).** A shader reads light → LUT → caster and projects the wedge entirely on the
  GPU. The real payoff; the reason to put casters + cull in textures at all.
- **Interim JS cast reading the textures.** Keep `castScreen` in JS but source caster rects + the in-range
  list from the LUT (JS already holds the arrays, so this only proves the *build*, not the GPU read).

**PICKED (staged):** landed the build-first fallback — the three textures are built + uploaded and the
light → LUT → caster indirection drives the cast off the CPU mirrors, verified ([D-1](deviations.md#d-1)).
The GPU read (raw ES-3.00 instanced cast with VTF) is the remaining C5. Rationale: the GPU read is the
first raw-shader work in the client and high-risk to one-shot; splitting keeps the data-structure proof
verified. — 2026-07-20

## F4 · Integer bitfield width — RGBA8UI (32) vs RGBA32UI (128)

- **`RGBA8UI` — 32 bits/texel.** Same memory footprint as today's unorm RGBA8; lifts the ceiling from 24
  (RGB, A taboo) to 32 (A is fine in an integer texture — no premultiply). Modest, cheap.
- **`RGBA32UI` — 128 bits/texel.** 4× the memory, but 128 separable lights in ONE texture — real dense-lights
  headroom, no MRT/second-target juggling.

**Pick:** _(pending)_ — likely `RGBA8UI` for the experiment (32 is already a clean win over 24 and matches the
current memory), with `RGBA32UI` as the drop-in when a real light count demands it. Decide in C5b.

## F5 · Multi-light OR without fixed-function blend ([I-7](issues.md#i-7))

Integer RTs can't additive-blend, so combining multiple lights' bits into the screen bitfield needs a plan:

- **One dirty light per frame (chosen default).** The experiment already casts ONE moved light per second;
  a single-light screen cast just writes bit `k` (overlapping casters of the same light are idempotent — no
  OR needed), and the merge shader ORs it into the world bitfield (which holds the other lights). Zero extra
  machinery for the common path.
- **Ping-pong RMW per light** for the seed / any multi-dirty frame: read the screen bitfield, OR the light's
  bit, write — one pass per light. Works, N passes.
- **Per-light bit-plane targets + a combine pass.** More memory + a pass; only if many-dirty-per-frame
  becomes common.

**Pick:** _(pending)_ — single-light screen cast + shader-OR merge for the steady path; RMW-per-light for the
seed. Revisit if the realtime set grows.

## F6 · Cast draw structure — instancing

- **One instanced draw per dirty light**, `instanceCount = count`, `gl_InstanceID` → `LUT[start + id]` →
  caster record (VTF). A uniform carries the light index + its bit. Simple; ≤ (dirty lights) draws/frame.
- **One instanced draw over the whole LUT** (`Σ count` instances), each instance carrying its light via a
  parallel per-instance attribute. Fewer draw calls, but needs the light-per-entry side array.

**Pick:** _(pending)_ — per-light instanced draw (matches the dirty-light cadence, no side array). Decide in C5c.
