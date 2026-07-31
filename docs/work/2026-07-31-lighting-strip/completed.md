# Completed — strip the lighting + shadow system

_The verification log: dated entries saying what landed and **how it was checked**. Append-only._

## 2026-07-31 · P0 — the four reference renders

Captured from the live client at commit `cddafe6f`, **before any code moved**. These are the record of
what the system delivered; after P2 the URLs below will still load, but they will render unlit, so
these images are the only remaining evidence.

| # | image | URL | what it shows |
|---|---|---|---|
| 1 | [`before/01-zoom1.jpg`](before/01-zoom1.jpg) | `:5174/?user=Claude&focus=100,50&zoom=1` | The default fixture. A light pool with soft radial falloff over a conifer stand, **every tree casting a projected silhouette**, the human pawn lit from the side, and the torch-lit wall structure at right. |
| 2 | [`before/02-zoom0.5.jpg`](before/02-zoom0.5.jpg) | `:5174/?user=Claude&focus=100,50&zoom=0.5` | Zoomed out. The **falloff ellipse** against unlit forest, shadow direction consistent across ~40 casters, and the lightmap holding up at lod 1 — the resolution ladder working. |
| 3 | [`before/03-torch-zoom2.jpg`](before/03-torch-zoom2.jpg) | `:5174/?user=Claude&focus=106,53&zoom=2` | The richest single frame: **torch glow behind conifers**, walls both lit and self-shading, per-tree cast shadows fanning by angle-to-light, and the wolf mover lit in the same pass. |
| 4 | [`before/04-shadow-edge-zoom2.jpg`](before/04-shadow-edge-zoom2.jpg) | `:5174/?user=Claude&focus=102,57&zoom=2` | The shadow **edge quality** at 16 texels/tile — the stepped boundary on the upper-left tree shadows. This is the artefact the deleted refine and the id map were both aimed at. |

### How they were captured, and why it took three attempts

Worth recording, because the obvious method returns a **plausible black image** rather than an error:

1. **`drawImage` after the fact returns black.** The context is `preserveDrawingBuffer: false`
   (confirmed from `getContextAttributes()`), so the drawing buffer is discarded at composite. The
   first capture wrote a 4 KB all-black JPEG and *succeeded* — caught only by opening the file.
2. **Patching `requestAnimationFrame` never fired.** The tab is `document.hidden`, so rAF is
   suspended: measured **0 calls in 10 s**. The same condition the shadow streams' harness was built
   tick-driven to work around.
3. **What worked:** drive a frame by hand with `__viewport.tick()` and `drawImage` **synchronously in
   the same task**, then encode and upload asynchronously (the 2D canvas retains the pixels once
   copied).

**Two of the four first captured mid-stream** — lit tiles with no sprites, because zone/texture
streaming had not resolved. Fixed by interleaving `tick()` with awaits until the downscaled frame hash
stopped changing. Both were re-shot and re-checked by opening the file. A capture that *returns a
size* is not a capture that shows the world.

Delivery avoided both base64-through-context and the download path: a localhost-only sink
(`scratchpad/shot_sink.py`, 127.0.0.1:8899, writes confined to `before/` by basename) receives the
blob straight from the page.

## 2026-07-31 · P0 — the one measurement that survived the cut

P0's measurement items were struck on the user's instruction ([D1](deviations.md)). One clean run had
already landed before the harness became the problem, and it is recorded here rather than discarded.

**Fixture:** `:5174/?user=Claude&focus=100,50&zoom=1`, 664 standing prims, 1 light at reach 16 tiles,
orbit on, 40 timed frames after 12 warm-up, GPU time by `EXT_disjoint_timer_query_webgl2`, draws
attributed by bound framebuffer.

| pass | render target | ms/frame |
|---|---|---|
| shadow-gather cold | `coldShadowRT` | **0.603** |
| display blit | default | 0.378 |
| world-lighting cold | `coldLightRT` | 0.119 |
| g-buffer bake | `SquareCache` | 0.007 |
| decay fade/splat | `decayRT` | 0.004 |
| | | **1.111 total** |

`GPU_DISJOINT_EXT` false, so the timings are valid.

**Cross-check:** the binary-shadow-ids P0 harness, written independently in another session, measured
**0.610 ms** for this same gather at N1 reach 16. Agreement to 1 % says the rig was measuring the
right thing — which is what makes the number worth keeping even though the phase was cut.

**The shape it confirms:** the gather is the pass, at **54 %** of frame GPU time with a single light —
and it scales with light count while nothing else here does. Two passes (receiver coarse/fine) did not
appear at all: they are dirty-gated and had already baked, which is the caching working as designed.
