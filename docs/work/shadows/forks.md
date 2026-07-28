# Forks — shadows

_Decision points + options + which we chose + why. Chronological. Most are the user's call for this
foundation slice (recorded so the rationale survives). All dated 2026-07-19 — the plan was refined the
same day it opened (the screen-hot/world-cold model, F8–F12, superseded the first pass's F7)._

---

## F1 · `shadow-cold` bit storage: start in RED, grow to the full texel — 2026-07-19 (ceiling updated 2026-07-20)

**Options:** (a) a 32-bit bitfield across the full RGBA texel (the target design); (b) start in the **RED
byte** and grow. **Chose (b).** This iteration: **6 lights in RED** — one channel keeps the first
pack/decode trivial. The **ceiling is RGB = 24** (the **alpha channel is never used for data** —
[F11](forks.md#f11), settled). Widening RED→RGB is the same float-mod shader over more bytes; beyond 24
needs more render targets (MRT = raw ES 3.00, deferred). See [D-1](deviations.md#d-1).

## F2 · Shadow lanes in RGB, never A — 2026-07-19

`shadow-hot`'s 3 screen-space lanes are **R/G/B, A left free**; `shadow-cold`'s bitfield is **RGB, A held
at 1**. The alpha channel is the premultiply/opacity lane on every draw/blit path — data there gets
multiplied into RGB and zeroed when "transparent". **A never works** for data; both RTs keep it out of
the data path. This is *why* the goal is 24 (3×8), not 32.

## F3 · Casters: pure billboards, no textures/outlines — 2026-07-19

**Options:** (a) the textured earcut silhouette (`outline`) with UV alpha, per
[`design/shadows.md`](../../components/client/webgl/design/shadows.md); (b) the **solid billboard quad**
projected to the ground. **Chose (b)** for the foundation — the projection + RT plumbing + pack are what's
being proven; the silhouette/UV layer drops on top later without changing the pipeline
([D-3](deviations.md#d-3)).

## F4 · Round-robin batches of 3 — 2026-07-19

3 lanes (`shadow-hot` RGB) ⇒ **3 lights cast per frame**. 6 lights → 2 frames; 24 → 8 frames. General
rule: `shadow-hot` of `L` lanes + a bitfield of `B` bits refresh in `ceil(B/L)` frames. See
[F12](forks.md#f12).

## F5 · Overlay: unique colour per bit, additive overlap — 2026-07-19

Each bit gets a distinct colour; a fragment sums its set bits' colours so **overlaps combine** and an
empty pixel is transparent. 6 colours this iteration (RED bits), 24 at goal (RGB). Makes the bitfield
legible — which light cast which shadow, and where they overlap.

## F6 · Lights via a `/coldlights` debug command — 2026-07-19

`LightRig` (and `/coldlight`) were deleted. **Chose:** a debug command `/coldlights [x y]` (+ `?coldlights`
URL) seeds a ring of 6 (growing to 24) around a tile, default (100, 50) — matches the existing
debug-command pattern, keeps the lights movable during bring-up, bakes no test data into boot. A plain
array on the viewport, not a revived rig.

## F8 · `shadow-hot` is SCREEN space, regenerated every frame — 2026-07-19

**The pivot that answers "why did the last attempt fail".** The nuke baked shadows **per-rect in
world-space**, forcing partial-shadow splits + which-rect-when-a-light-moves bookkeeping. **Chose:** cast
shadows in **screen space** into a viewport-sized `shadow-hot`, rebuilt **every frame**. The whole
viewport is one surface — no rect boundaries to split across, no per-rect dirty tracking. Rebuilding each
frame means it's never stale and never needs reprojection (a moved light just re-casts next frame). The
cost — re-casting every frame — is cheap for a handful of billboard quads and is what buys the
simplicity. This is the load-bearing decision; everything else hangs off it.

## F9 · `shadow-cold` is world-space but holds LIGHTS, not rects — 2026-07-19

`shadow-cold` must persist across frames and stick to the world (so shadows don't swim when the camera
pans/zooms), so it lives in the **world-space toroidal layout** the other G-buffer RTs use. But unlike
them it **stores no per-rect geometry** — each pixel is a **bitfield of which lights shadow that world
point**. So it isn't *baked* per-rect (no dirty loop); it's **filled by copying** the screen-space
`shadow-hot` into it ([F10](forks.md#f10)). "Cold" is provisional — the round-robin fill is warm-tier
behaviour ([D-6](deviations.md#d-6)).

## F10 · Screen→world bridge = 4 copy commands (the toroidal wrap) — 2026-07-19

The viewport window wraps around the toroidal buffer's seams, so the screen-space `shadow-hot` rectangle
lands as **up to 4 rectangles** in the world-space layout (H seam × V seam → 4 quadrants). **Chose:**
translate with **4 copy commands**, one per wrapped quadrant, reusing the cache's existing window→buffer
wrap math. Each copy is a **pack**: reads `shadow-hot` RGB + prev `shadow-cold`, sets the frame's 3 bits,
writes back. This single bridge is what lets us cast in the easy space (screen) and store in the required
space (world) — sidestepping both partial shadows and light-move rect bookkeeping.

## F11 · Bitfield ceiling: 24/RT (RGB), A never used — SETTLED 2026-07-20

**Final (user decision):** **24 separable lights per RT — RGB only, the alpha channel is NEVER used for
data.** Premultiply mangles A, and the verbatim-write workaround to reclaim it isn't worth the risk (it's
"error-prone"). So drop the earlier "32 via A" idea entirely. Storage is a **unorm RGBA8** bitfield with
**A held at 1** (premultiply is a no-op — proven by `bitfield-rt`, archived).
`shadow-hot` stays a float RGBA8, RGB = 3 lanes (A avoided). Throughput unchanged: **≥3 hot/frame ⇒ ≥24
cold cyclable in ~8 frames**. Beyond 24 separable lights → **more render targets** (24 each), but MRT is
gated behind raw ES 3.00 shaders ([F14](#f14)), deferred. See [`design/rendering-platform.md`](../../components/client/webgl/design/rendering-platform.md).

## F13 · Shadow geometry: CPU place + cull, GPU rasterise — 2026-07-19

**Options:** (a) build the projected shadow-quad vertices on the **CPU** and upload; (b) project in a
**GPU vertex shader** (instanced). **Chose a hybrid, weighted to (a) for now.** Fill (rasterisation) is
always GPU. Projection math is trivial either way, but at the foundation's small counts (round-robin ×
in-range culling) CPU-placing the geometry is simplest in Pixi and the per-frame upload is tiny. The real
CPU job is **culling** the (light, prim) pairs to those within a light's reach — a spatial query that
belongs on the CPU regardless. So: **CPU culls + places the small batch; GPU rasterises.** Revisit
vertex-shader projection only if a profile shows the JS build/upload cost at scale. (The future textured
silhouette fan — [D-3](deviations.md#d-3) — is variable per-caster geometry, which *further* favours CPU
placement.)

## F14 · Shaders are GLSL ES 1.00 (float-mod); ES 3.00 deferred — decided 2026-07-20, CORRECTED same day

**Decision (corrected):** the shadow shaders are **GLSL ES 1.00** and bits use **float-mod**; **WebGL2 is
kept as a context baseline** but not for ES 3.00. Durable stance:
[`design/rendering-platform.md`](../../components/client/webgl/design/rendering-platform.md).

**The reversal:** this fork first said "author `#version 300 es`, ES 3.00 is free because the app already
runs WebGL2." **That premise was wrong**, proven by executing
`bitfield-rt` (archived): Pixi v8's high-shader system compiles **ES 1.00 with
WebGL1-compat shims** even on a WebGL2 context (`#define in varying`, `gl_FragColor`, no `#version 300
es`), so `uint`/bitwise/integer-textures/MRT are **not** available through the bit system — a `uint` in an
overlay bit failed to compile. ES 3.00 is reachable only by **hand-writing raw `GlProgram`s** (bypassing
the stock bits) — a real cost. **So:** stay on ES 1.00 + float-mod (proven exact on rgba8); defer ES 3.00
(and with it integer textures + MRT) until a concrete need — the many-lights **MRT** scale, or >32-bit
fields — justifies hand-written raw shaders.

## F12 · 3 hot/frame ⇒ ≥24 cold, ~8-frame cycle — 2026-07-19

The throughput target: **3 hot lights per frame** (one `shadow-hot` RGB fill), cycled round-robin ⇒ **24
cold lights** refreshed every **~8 frames** (~130ms @ 60fps). A cold light's shadow is at most that stale.
This is exactly the design's warm round-robin cadence, reached via the screen-hot/world-cold copy instead
of the design's scatter→ping-pong.
