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

## 2026-07-31 · P1 — the renderer goes unlit (consumers cut, nothing deleted)

**What landed.** The display blit lost every lighting term — the cold+hot lightmap sum, the
de-quantisation, `ambient × AO`, the decay glow and the emissive add — leaving `albedo.rgb × alpha`.
`Viewport` stopped binding the lightmaps/decay and stopped issuing `shadows.tick()`, which is what
drove the gather, lighting, receiver and decay passes. The blueprint preview, which sampled the live
lightmaps so a blueprint dragged past a torch glowed, now draws unlit.

Dead-by-consequence and removed with it: `uLightEnable`, `uAmbient`, `uAoStr`, `uEmissiveBoost`,
`uLightQuant`, `uLCols/uLRows/uLWinCol/uLWinRow/uLSlot/uDSlot`, the `lightTexel` and `decaySample`
functions, the `vWorld` varying, the `__ao` and `__emissive` debug hooks, `aoStrength`/`emissiveBoost`,
and four now-unused imports. **`ShadowGather` itself is untouched and still compiles** — that is the
point of doing this before P2 ([F3](forks.md#f3)).

### The measurement — what the lighting system cost

Same fixture, same method, measured **both ways** by stashing the P1 diff: 60 frames after 15 warm-up,
`gl.finish()` at both ends so the number prices the whole frame (CPU + GPU), 5 repeats, median.

| build | draws/frame | ms/frame (median) | |
|---|---|---|---|
| lit, static scene | **4** | **0.508** | min 0.497, max 0.672 |
| lit, light orbiting | 4 | **0.675** | min 0.647, max 1.198 |
| **unlit (P1)** | **1** | **0.045** | min 0.040, max 0.053 |

**The lighting system was ~91 % of a static frame and ~93 % of a moving-light frame.** The replacement
inherits a budget of roughly **0.46–0.63 ms/frame** at this scene's density before it costs more than
what was removed.

Two honest qualifiers. This is the **content scene** — one authored torch — not the N=16 stress case;
the shadow streams' harness numbers (gather alone at 4–7 ms with 16 lights) are the right reference for
how it scales. And the frame is **dirty-gated**, so "static" is the cheap case by design: the 4 draws
are the steady state, not the work a change triggers.

### Verified

- **Page load at the fixture, zero console errors.** tsc cannot see GLSL, so this is the only real gate
  — and the blit was rewritten, so a silent GLSL break was the live risk.
- **The unlit render is correct**: [`after/01-zoom1-unlit.jpg`](after/01-zoom1-unlit.jpg). Trees,
  bushes, the human pawn, the wolf, the walled structure and the torch sprites all draw; sprite z-order
  is intact (the warm-over-cold painter's key is untouched); tile art reads correctly at full albedo.
  Compare [`before/01-zoom1.jpg`](before/01-zoom1.jpg) — same camera, same content, no light.
- **Draw count fell 4 → 1**, counted by wrapping `drawElements`/`drawArrays`. The plan predicted 2
  remaining; the truth is **1**, because the G-buffer bakes are dirty-gated and idle once the scene has
  settled. The prediction was wrong in the harmless direction and the measured number is what stands.

### Both fixtures, and the overlays

- **Zoom 0.5** — [`after/02-zoom0.5-unlit.jpg`](after/02-zoom0.5-unlit.jpg): biomes (sand, dirt,
  grass), the forest, the walled structure and the movers all draw. The white block at the pawn is its
  unresolved-lod geo fallback and appears in the **lit** before-image at the same spot, so it predates
  the strip.
- **Selection works**: a synthesised pointerdown/up on the pawn selected `p:813694981`.
- **The outline overlay draws** — populating it took the frame from 1 draw to 2, `getError() == 0`.
- **The blueprint overlay draws** — same, 1 → 2, `getError() == 0`. Its lighting argument is now always
  `null`, which was already the supported branch whenever the lightmaps were absent.

**One honest note on method.** Driving `Viewport.tick()` by hand bypasses `WorldScene.update()`, which
is where `syncOutlines()` and the mover tick live — so the *first* selection test showed no outline
draw and that was my harness, not a regression. Both overlays were then populated directly to exercise
their draw paths. Worth writing down: in this manual-tick rig, "nothing drew" can mean "the scene
update never ran".

## 2026-07-31 · P2 — the machinery is deleted

**4 348 lines gone**: `shadowGather.ts` (3 000) and `coldShadowData.ts` (1 348), both removed whole
rather than split — see [D2](deviations.md), which records that [F5](forks.md#f5)'s premise was wrong.

### F5 was wrong, and checking it first is what made this phase small

F5 planned to *keep* the primitive-graph writer because "sprites, z-depth and selection all read it".
Verified before touching anything:

- **Sprites and z-depth never read it.** No `uData`, no `usampler` anywhere in `mrtBakeShader.ts` or
  `SquareCache.ts` — the G-buffer bake does not sample the data texture at all.
- **One non-lighting consumer existed**: `thingAt()` → `tightBoxFor()`, six lines over
  `definitionFor` + `tightBoxOf`.
- **And its data had another source**: `TextureResolver.opaqueBBox(stem)` returns the same silhouette
  as frame fractions, which the record path then *quantised to even units* for the shadow card.

So `tightBoxFor` was reimplemented in `Viewport` straight off the resolver — **finer** than what it
replaced, since hit-testing has no reason to inherit the shadow card's 2-unit grid.

### Freed

| target | size |
|---|---|
| `coldShadowRT`, `hotShadowRT`, + both `Prev` | 2 MiB × 4 = 8 MiB |
| `decayRT` | 1 MiB |
| `receiverCoarseRT` | 2 MiB |
| `receiverFineRT` | 8 MiB |
| `coldLightRT`, `hotLightRT` | 32 MiB × 2 = **64 MiB** |
| **the unified data texture** (1024×1024 `RGBA32UI`) | **16 MiB** |
| | **99 MiB** |

Computed from the allocation sites as they were deleted. The README estimated 83 MiB and was right
about the render targets — it just did not count the data texture, which went with
`coldShadowData.ts`. P0's empirical before-measurement was struck by the user ([D1](deviations.md)),
so this is arithmetic rather than a reading, and is labelled as such.

### Also removed, because their reason to exist went

`setMaxCardTiles` (the walk dilation) and `setTileKinds` (tiles entering presence) fed the gather and
nothing else; `tileKindDirty`; the `/shadows` chat command; the `shadow-cold` overlay channel (it named
a render target that no longer exists, so offering it would have been a menu entry that silently shows
nothing); and **the cursor light** — a 1 px invisible warm prim re-baked on *every pointer move* to
carry a `light` payload nothing reads.

### Verified

| check | result |
|---|---|
| page load, both fixtures | **zero console errors**, `getError() == 0` |
| render vs P1 | [`after/03-zoom1-deleted.jpg`](after/03-zoom1-deleted.jpg) — identical; deletion is invisible on screen |
| lighting debug hooks | all **15** probed (`__gather`, `__cold`, `__corridor`, `__orbit`, `__torch`, `__tilt`, …) return `undefined` |
| selection, mover | click selects the pawn |
| selection, cold thing | `thingAt` at a conifer's centre → prim 138; **at its corner → `null`** — the box is still silhouette-tight, not the full quad |
| cursor-light removal | 20 synthesised pointer-moves leave the warm prim count at 3, unchanged |
| draws/frame | **1** |

**Frame cost fell 0.045 → 0.028 ms** (median of 5, spreads do not overlap). P1 had already stopped
calling `shadows.tick()`, so the remaining cost was `moverDirty` rewriting records per mover per frame
plus the cursor light's per-move re-bake — both of which this phase removed.

### The docs-check hook caught what the deletion broke

The commit was **blocked** by the pre-commit gate: 7 links across 5 historical work streams pointed at
`shadowGather.ts` / `coldShadowData.ts`, which had just stopped existing.

De-linked rather than reworded — the prose is still true *history* (those streams really did work on
those files), so the filename survives as inline code while the link, which asserted a live file, does
not. `docs/work/README.md` included, so the index no longer promises a file the tree lacks.

Worth recording as a win for the guard: deleting 4 348 lines of code silently invalidated
documentation five streams away, and nothing in the change itself would have surfaced that.
