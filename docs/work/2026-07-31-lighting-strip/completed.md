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

## 2026-07-31 · P3 — the data-texture bands retired

**P3 turned out bigger than planned, in the direction D2 predicted.** The phase was written to un-wire
three light bands and leave the primitive graph standing. With `coldShadowData.ts` deleted in P2, the
grep for readers came back **empty for every band, not just the light ones**:

```
prim_presence | light_presence | light_data | PRESENCE_BASE | DEF_BASE | dataMirror
  → 2 hits, both COMMENTS (SquareCache.ts:569, WorldBridge.ts:508). Zero code.
```

So the **entire unified data texture** is dead, not a subset of it — `definition_data`, `prim_data`,
`billboard_data`, `light_data`, both `light_presence` sets, `prim_presence`, plus the `shadow-cold`,
`shadow_dirty` and `light_presence_cold` maps.

**431 lines removed from `VARIABLES.md`** — the whole `## Cold shadow data textures` section. Not
marked obsolete, not left with a banner: this file is AUTHORITATIVE and holds current truth only, and
a layout spec for a texture that does not exist is worse than no spec, because a reader has no way to
tell it is describing the past. In its place, fourteen lines that say what happened, that nothing reads
or writes any of it, and where the successor is designed.

**446 lines total, and the gate corrected me twice on the way.** My first attempt left a tombstone
section reading "RETIRED 2026-07-31" with a pointer to the successor design. `docs-check` rejected the
phrase *"superseded by"* — _"'superseded by' in an authoritative doc: describe the current shape, not
what it replaced"_ — and reading the rule properly, the whole tombstone was the same mistake at larger
scale. The file's own convention is a one-entry line under **`## Removed`**, exactly as `valid_at`,
`cold_reference` and `hot_reference` are handled. So the section is simply gone and the retirement is
one paragraph there.

The successor pointer moved out with it: "where the replacement is being designed" is not a fact about
current variables, and belongs in the component's `current/` doc (P4) and the seam doc (P5).

**`## Textile slot grid` was deliberately KEPT.** `SLOTS_X/Y`, `TEXTILE_*`, `SQUARE`/`UNIT` and the lod
ladder are live: `squareMath.ts` exports them and the G-buffer bake rides them. It sits directly above
the deleted section and would have been easy to take with it.

### Verified

- `rd docs-check` green across 461 files — no dangling internal link into the removed section.
- The two surviving mentions are prose in code comments describing history, not references to a live
  band.
- Page load clean; the render is unchanged, which it must be — this phase touched no code.

**P3's fourth item — "verify the data texture end to end" — is moot and recorded as such**: there is
no data texture to verify. Its acceptance ("the scatter still delivers records, the mirror matches")
described a system P2 deleted.

## 2026-07-31 · P4 — the documentation retired

**Four design/intent docs out of the repo** to `../archive/`: `design/lighting.md`,
`design/shadows.md`, `intent/shadows.md`, `intent/tiered-lighting.md`. Out of the repo rather than into
an in-repo `archive/`, per the convention — an in-repo one gets read as current.
`components/client/webgl/intent/` is now empty and `design/` holds only what still exists
(`rendering-platform.md`, `de-lighting.md`).

**Nineteen work streams archived**, not eight. The plan's count came from `issues.md` I6, which
listed only the recent ones; the real set is every open/paused/blocked stream whose subject was the
deleted system, going back to `2026-07-21-shadow-bitfield`. Moved to
`../archive/lighting-strip-superseded/` and recorded in `docs/work/README.md` as **superseded, not
delivered**.

**Two streams kept open deliberately**: `2026-07-29-analytic-wall-normals` and
`2026-07-29-marigold-linked-normals` are **art pipeline**, not renderer. Their output — the normal and
depth maps — survives the strip ([F4](forks.md#f4)) and the rework wants it. I6 flagged this and P4
decided per stream rather than sweeping.

**`current/` now exists** and says plainly what the renderer does: unlit, one draw, 0.028 ms/frame,
with the G-buffer channels available at the seam and a pointer to where the successor is designed.

### What the gate caught, three times

Archiving is a link-breaking operation and `docs-check` refused the commit at each step:

| | broken | fix |
|---|---|---|
| the four component docs | 4 links from `components/client/webgl/README.md`, 6 from work streams | rewrote the README index; de-linked the rest |
| the 19 streams | **67** cross-references between streams | de-linked to inline code — the prose is true history, the link asserted a live file |
| `current/README.md` | — | _"missing freshness stamp — a cache with no age is unsafe to plan from"_ |

The last one is the sharpest rule in the tree and I had not met it: a `current/` doc without a
`Last updated` / `verified @ <sha>` is a claim about the present with no way to check its age. Stamped.

**354 files, green.** Down from 461 — and the repo-wide over-long-item warning fell from 244 to 173
purely as a side effect of the archive.
