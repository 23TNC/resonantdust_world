# Forks — shadows

_Decision points + options + which we chose + why. Chronological. Most are the user's call for this
foundation slice (recorded so the rationale survives). All dated 2026-07-19 — the plan was refined the
same day it opened (the screen-hot/world-cold model, F8–F12, superseded the first pass's F7)._

---

## F1 · `shadow-cold` bit storage: RED byte now, RGB (24) goal — 2026-07-19

**Options:** (a) a 32-bit bitfield across the full RGBA texel (the target design); (b) start in the **RED
byte** and grow to **RGB** (24 bits). **Chose (b).** This iteration: **6 lights in RED**. Goal: **24
lights in RGB** — *not* 32, because **A never works** ([F11](forks.md#f11)). One channel keeps the
pack/decode trivial to bring up; RGB is the same shader over 3 bytes. See [D-1](deviations.md#d-1).

## F2 · Shadow lanes in RGB, never A — 2026-07-19

`shadow-hot`'s 3 screen-space lanes are **R/G/B, A left free**; `shadow-cold`'s bitfield is **RGB, A held
at 1**. The alpha channel is the premultiply/opacity lane on every draw/blit path — data there gets
multiplied into RGB and zeroed when "transparent". **A never works** for data; both RTs keep it out of
the data path. This is *why* the goal is 24 (3×8), not 32.

## F3 · Casters: pure billboards, no textures/outlines — 2026-07-19

**Options:** (a) the textured earcut silhouette (`outline`) with UV alpha, per
[`design/shadows.md`](../../components/client/pixijs/design/shadows.md); (b) the **solid billboard quad**
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

## F11 · Goal = 24 lights (RGB), not 32 — 2026-07-19

The design says 32-bit. **Chose 24** because **A never works** (F2): the bitfield lives in RGB = 3 bytes ×
8 bits = 24. Theory: **≥3 hot lights/frame** is achievable, ⇒ **≥24 cold** lights cyclable in ~8 frames.
If A ever proved usable (it won't, on the premultiply paths), 32 would follow for free — but the plan
targets 24.

## F12 · 3 hot/frame ⇒ ≥24 cold, ~8-frame cycle — 2026-07-19

The throughput target: **3 hot lights per frame** (one `shadow-hot` RGB fill), cycled round-robin ⇒ **24
cold lights** refreshed every **~8 frames** (~130ms @ 60fps). A cold light's shadow is at most that stale.
This is exactly the design's warm round-robin cadence, reached via the screen-hot/world-cold copy instead
of the design's scatter→ping-pong.
