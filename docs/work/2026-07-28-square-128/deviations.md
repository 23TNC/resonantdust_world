# Deviations — SQUARE 128

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

## D1 — P0 item 1: no cross-check against "the browser's reported GPU memory" {#d1}

**Planned:** "a table of art / lightmap / shadow / tile bytes whose total matches the browser's reported
GPU memory within 10%."

**Done instead:** the table is built by walking the live `RenderTarget`/texture objects in the page and
multiplying each attachment's real dimensions by its real format width. No browser total is compared.

**Why:** WebGL2 exposes no GPU-memory query — `WEBGL_debug_renderer_info` gives vendor/renderer strings
only, `performance.memory` is the JS heap, and `chrome://gpu` cannot be read from page script. The
criterion named a check that does not exist at this vantage point. Writing the plan I assumed a total was
obtainable; it is not.

**What this costs:** the table cannot catch an allocation the walk never visits (a texture held somewhere
outside `__viewport`/`__gather`, or driver-side overhead). It *does* catch mis-sized and mis-formatted
maps, which is what the phase is actually for — and it already caught two ([I3](issues.md)). The
projections are relative (same walk before and after), so an unvisited allocation cancels out.

## D2 — P0 item 2: harness drives `tick` directly instead of running under rAF {#d2}

**Planned (by inheritance):** the calibrated orbit harness from `2026-07-27-plane-intersection`, which
ran under `requestAnimationFrame` for a fixed frame count.

**Done instead:** `ShadowGather.tick` is called in a synchronous loop.

**Why:** in a tool-driven session the game tab is backgrounded, and Chrome **suspends rAF entirely** for
hidden tabs — `document.visibilityState` reads `hidden` and zero callbacks fire. Three attempts stalled
(45 s CDP timeouts) before this was diagnosed. Clicking the page did not restore it.

**Why this is an improvement, not a compromise:** it gives an exact frame count with no vsync jitter and
no dependence on window focus. All three harness bugs the previous stream fixed remain fixed — target
discrimination, phase from the frame index, fixed frame count — and the reach calibration passes
(0.134 → 0.668 → 0.845 ms). Adopt it as the default fixture for the rest of the stream.

**Caveat to carry:** it times the gather + lighting draws in isolation, not a whole app frame. Comparing
against a figure produced by the *old* rAF harness would be apples-to-oranges; every comparison in this
stream is same-harness before/after.

## D3 — P0 item 2: ±0.01 ms tolerance replaced by 3 repeats with reported spread {#d3}

**Planned:** "three numbers, each stable to ±0.01 ms across two runs."

**Done instead:** 3 repeats per reach, reporting the mean and the full spread.

**Why:** the tolerance is unachievable and I set it without evidence. On a workload proven identical
(dirty tiles byte-identical at 217/392/465 across repeats), reach 4 measured 0.110, 0.120 and 0.142 ms —
a 0.032 ms spread, 3× the demanded tolerance, at 26 % of the mean. GPU timer queries on sub-millisecond
draws simply do not resolve to 0.01 ms.

**What this costs:** the stream's later "within noise of the baseline" checks must clear the **measured
spread** (~0.07 ms absolute at reach 8), not 0.01 ms. That is a weaker gate than planned, and it is the
real one. A change smaller than that spread cannot be claimed from this harness at all — it would need
more repeats or a bigger workload.
