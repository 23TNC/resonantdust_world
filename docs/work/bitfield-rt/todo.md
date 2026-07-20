# Todo — bitfield-rt (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. See
[`README.md`](README.md) for the experiment and [`issues.md`](issues.md) for the guards each step must
respect._

---

## E1 · The world-space integer bitfield RT — 2026-07-20

- [ ] Ensure the renderer is **WebGL2** and the experiment shaders are **`#version 300 es`**
      ([F14 in shadows](../shadows/forks.md#f14), [`design/rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md)).
- [ ] Add a `lightmap-cold` RT in the cold `SquareCache`'s **toroidal world-space** layout (share the
      window/slot geometry so it pans + scales with the other composites). Format **`RGBA8UI`** (integer),
      sampler **nearest**, **no blend**, **linear** color space (not sRGB) ([F1](forks.md#f1),
      [I-1](issues.md#i-1)…[I-4](issues.md#i-4)).
- [ ] List it in `Viewport.renderTextures()` so `/overlayRT lightmap-cold` (and `/showRT`) can select it.

## E2 · Fill pass — one-hot bit per rect for the zone at (100,50) — 2026-07-20

- [ ] A single fill pass (`#version 300 es`) over the RT: per fragment, resolve which **world tile-rect**
      it belongs to, compute a stable rect index within the target zone (`localX + localY*16`, `0..255`),
      set **bit `index mod 24`**, output the packed `uvec4` (bits 0–7→R, 8–15→G, 16–23→B, A=0). Fragments
      outside the 16×16 zone at `(100,50)` write **0** ([F2](forks.md#f2), [F3](forks.md#f3)).
- [ ] No blend, write replaces (read-modify-write not even needed here — each rect is written once).
      Confirm the write lands per-rect (not one flat value across the buffer).

## E3 · Decode overlay — bitfield → 24 colours — 2026-07-20

- [ ] Add a **bitfield-decode** overlay mode; route `lightmap-cold` to it in `overlayModeFor`.
- [ ] In `overlayShader` (`#version 300 es`, `usampler2D` + `texelFetch`): read the rect's bits, find the
      set bit with real `uint` bitwise (`(bits >> i) & 1u`), index a **24-colour palette**, output that
      colour. Transparent (or black) where no bit is set (outside the zone) ([F4](forks.md#f4)).

## E4 · Verify — 2026-07-20

- [ ] In-browser at `?focus=100,50`, `/overlayRT lightmap-cold`: confirm a **16×16 grid of 256 rects**,
      each coloured by its bit, **all 24 colours present**, no black/smeared/off-by-one rects.
- [ ] **Pan** the camera → each rect keeps its colour (world-space RT). **Zoom** in/out → rects scale
      cleanly (nearest), no bleeding between rects. These prove the RT survives the toroidal pan +
      nearest reproject, not just a static frame.
- [ ] If any [FAIL-mode](README.md) shows, record which in [`issues.md`](issues.md) with the cause it
      points at — that's the whole deliverable of the experiment.

## E5 · Ping-pong read-modify-write proof — 2026-07-20

_The accumulate path `shadow-cold` actually needs, proven on the known-good storage layer from E1–E4
([I-7](issues.md#i-7), [shadows I-8](../shadows/issues.md#i-8))._

- [ ] Two world-space bitfield buffers, **A** and **B** (ping-pong). Pass 1: write bit `i` per rect into A
      (as E2). Pass 2: a shader that **`texelFetch`es A** (source) and writes **`bits_A | (1u << j)`** into
      **B** (destination) — source ≠ destination, so no framebuffer feedback loop.
- [ ] Decode **B** via the overlay → each rect shows **both** colours (bit `i` + bit `j`) combined. That
      proves: bits **survive a read-back**, the **OR preserves** the existing bit, and **ping-pong** avoids
      the in-place feedback loop — the exact mechanism `shadows`' pack inherits.
- [ ] **Carry-forward check:** the pass-2 shader must write *every* texel (OR with 0 where no new bit), so
      confirm rects that get no pass-2 bit still keep their pass-1 bit in B (no dropped texels).

## E6 · Graduate — 2026-07-20

- [ ] On PASS (E4 storage + E5 RMW): note the proven config (format / sampler / blend / colorspace / pack +
      decode shape / ping-pong) in [`shadows/issues.md`](../shadows/issues.md) so `shadow-cold` inherits it,
      and archive this folder.
