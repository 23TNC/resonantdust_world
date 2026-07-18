# Blockers — prim-batching

_Things stopping a `todo` item from landing. Each: what's blocked, why, the plan to clear it. Move the
item back to `todo` once cleared (leave a dated resolution line here)._

---

## B1 · No reliable in-browser draw-call measurement (blocks P1 baseline) — 2026-07-18

**✅ RESOLVED 2026-07-18 (user call): we don't need a precise measurement to implement correctly.**
Informal baseline from the DebugPanel: **~460** draw calls, of which **~200** is the shadow pass
(deferred). The batching win will be self-evident when the mesh geometry lands — measure the delta then,
not before. P1 unblocked (the live-baseline bullet de-scoped to "the panel's ~460/~200 is the
before-number"). The panel-accuracy sub-question is likewise parked — the after/before delta is what
matters, and the panel measures it consistently.

**Blocked.** _(historical)_ P1's "measure the honest draw-call breakdown at `x=100,y=50`" can't be
trusted yet.

**Why.** The ad-hoc probe (patch `gl.drawElements` from the console via
`document.querySelector('canvas').getContext(...)`) returned **2.5 draws/frame** in an empty ocean
while the in-`tick` tally showed **157** — almost certainly a wrong/idle canvas or context reference
(there may be more than one canvas; PixiJS's `renderer.gl` need not be the one `querySelector` returns),
compounded by REPL syntax errors. So neither the console probe **nor** the panel's number is confirmed
trustworthy — and the whole stream is measured against P1's before-number, so a bad baseline poisons
every later "we improved it by X" claim.

**Plan to clear.** Don't measure ad-hoc. Wire a **temporary, flag-gated** per-pass tally into the code
against the SAME context the app renders through (`renderer.gl` — the object `DrawCallCounter` patches),
logging deltas around `warm.bakeDirty` / `map.bakeDirty` / the shadow pass, with stage/UI = panel-total
− tick-sum. Cross-check the sum against `DrawCallCounter.readAndReset()` so the two agree (that
agreement also answers the still-open "is the panel accurate?" question). Capture idle + a slow pan at
`x=100,y=50`. Remove the instrumentation before P2.

**Open sub-question (surfaced, not answered):** is the DebugPanel's draw-call count accurate at all?
`DrawCallCounter` patches `renderer.gl.drawElements`/`drawArrays` and `readAndReset()`s once per
`app.ticker` frame — looks correct, but it counts **RT passes too** (bake/shadow/blit), so it is "total
GPU draws," not "draws to the screen." Confirm it ticks 1:1 with renders (no multi-frame accumulation)
as part of clearing this blocker.
