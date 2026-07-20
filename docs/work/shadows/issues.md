# Issues — shadows

_Problems + candidate solutions + which we chose + why. Chronological. Seeded on open with the technical
gotchas this plan already anticipates (several are hard-won from the nuked build), so they're resolved
deliberately rather than re-discovered. Move to "resolved" with a date as they land._

---

## I-1 · Zoom-reprojecting a bitfield garbles the bits — open, 2026-07-19

`shadow-cold` scales with the cache window on zoom, but a **bitfield can't be bilinearly resampled** —
interpolating byte values invents garbage bits (the nuked build hit this and skipped shadow channels in
reproject). **Solution:** reproject `shadow-cold` **nearest** (not bilinear) on a zoom, and lean on the
every-frame `shadow-hot` regeneration + round-robin to **refill** at the new scale within a few frames.
So a zoom shows correct shadows immediately at nearest resolution, sharpening as the round-robin catches
up. This is why casting-every-frame ([F8](forks.md#f8)) is what makes zoom a non-issue.

## I-2 · The 4-copy must land pixel-exact in the toroidal buffer — open, 2026-07-19

The screen→world copy ([F10](forks.md#f10)) must place `shadow-hot` at the **exact** world position or
shadows drift as the camera pans. **Solution:** reuse the cache's existing window→buffer wrap +
pan-snap math verbatim (the same the composites already pan by) so `shadow-cold` registers 1:1 with
`albedo-cold` et al. Nearest sampling + whole-pixel pan snap; don't invent a second coordinate path.

## I-3 · A moved light leaves a stale shadow until its next refresh — open, 2026-07-19

Round-robin refreshes a light only every ~⌈N/3⌉ frames; between refreshes, a light that **moved** still
has its **old-position bit** set in `shadow-cold`. **Solution:** on any light-set change (`/coldlights`
re-seed, a light move), **clear `shadow-cold`** and reset the round-robin cursor so no ghost lingers
(P4). Fine at debug scale; real static lights rarely move, which is the whole premise of the cold tier.

## I-4 · `shadow-hot` channel union without overflow — open, 2026-07-19

Multiple casters for the **same** light must **union** into that light's channel, not sum past 1. Use a
per-light channel-write shader (`outColor = uChannel`, a 1 in one lane) with **`max` blend** — overlapping
caster quads clamp at 1. `add` would overflow; the tiered-lighting anti-goals call this out.

## I-5 · Premultiply corrupts a data byte — now only a `shadow-hot` concern, updated 2026-07-20

Premultiply (`RGB × A`, zeroing data where A=0) is a **float-RGBA8 / blend / batch-shader** artifact.
- **`shadow-hot`** IS a float RGBA8 with `max`-blend (to union casters per light), so it keeps data in
  **RGB, A free** ([F2](forks.md#f2)) — premultiply still applies here.
- **`shadow-cold`** as an ES 3.00 **integer** texture (`RGBA8UI`, [F14](forks.md#f14)) is **not**
  blended/premultiplied, and its pack is an explicit read-modify-write — so **all 4 bytes, A included, are
  usable data** (→ 32/RT, [F11](forks.md#f11)). If `shadow-cold` is instead kept a unorm RGBA8, reclaim A
  with the nuked build's **verbatim non-premultiply Mesh blit** ("bitfield linchpin").

## I-6 · Bit set/test — real `uint` bitwise (GLSL ES 3.00) — updated 2026-07-20

Since [F14](forks.md#f14) the shadow shaders are `#version 300 es`, so bits use **real integer ops**, not
the ES-1.00 float-mod emulation:
- **`shadow-cold` as `RGBA8UI` / `usampler2D`:** pack = read-modify-write via `texelFetch` (no blend) —
  `bits |= (mask << shift)`; test/decode = `(bits >> i) & 1u`. Exact, no `n/255` discipline.
- **if `shadow-cold` is kept a unorm RGBA8** (simpler Pixi plumbing): unpack in-shader
  (`uint b = uint(v*255.0 + 0.5)`), do the same bitwise, repack (`float(b)/255.0`). Still real ops.

The retired ES-1.00 shape — `mod(floor(byte*255 / exp2(i)), 2.0)` to test, `+ exp2(i)/255` to set — is
kept only as the fallback if some shader must ever stay ES 1.00; it isn't the plan anymore.

## I-7 · Casting is in screen space — need caster + light screen coords — resolved-by-design, 2026-07-19

Projection happens in **screen** space now ([F8](forks.md#f8)), so both the light and the caster's
billboard corners must be mapped world→screen (current pan+zoom) before the radial ground projection is
rasterised into `shadow-hot`. This is the same world→screen transform the display mesh already uses — no
new math, just apply it before rasterising rather than baking in rect-local world coords.
