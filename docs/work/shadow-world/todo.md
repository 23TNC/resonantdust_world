# Todo — shadow-world (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Edits the
archived `shadow-cast` code (`shadowCast.ts` / `shadowCastShaders.ts`), not a rewrite. See
[`README.md`](README.md) for the two changes, [`forks.md`](forks.md), [`issues.md`](issues.md)._

---

## W1 · Move `shadow-a`/`shadow-b`/`mask` to the world-space buffer — 2026-07-20

- [ ] Size the three RTs to the cache's fixed buffer (`fixedCW × fixedCH`) at the composite resolution,
      `nearest`, A=1; share the cache's **window origin + `slotPx` + pan** so they register 1:1 with
      `albedo-cold` et al. (Expose what's needed from `SquareCache`/`Viewport`.)
- [ ] Reproject `shadow-a`/`-b` with the window on zoom (**nearest** — bitfield) then re-cast to refill
      ([issues I-2](issues.md#i-2), [shadows I-1](../shadows/issues.md#i-1)).

## W2 · Cast in world space → world `mask` — 2026-07-20

- [ ] Project each in-radius prim's billboard from the light to the **ground (z=0) in WORLD coords** (drop
      the world→screen step from shadow-cast) → a world-space shadow quad.
- [ ] Map world → fixed-buffer slot (the same window→slot transform the composites bake through) and
      rasterise the quads into the world-space `mask` (`Graphics`, union). **Toroidal wrap:** a quad across
      the seam writes up to 4 pieces — reuse the cache wrap ([F3](forks.md#f3)); *bring-up may write just
      the visible window first.*

## W3 · Combine + ping-pong (world-space) — 2026-07-20

- [ ] Combine reads world `src` bitfield + world `mask` → writes world `dest`: clear bit k, set where
      masked, carry the other 4 bits ([issues I-1](issues.md#i-1)). Swap `src`↔`dest`. Source ≠ destination
      (no feedback loop). Same float-mod RMW as shadow-cast, now over the world buffer.
- [ ] Keep the per-second move + dirty-queue (re-cast only the moved light) and the light dot + radius-ring
      **markers** (project world→screen for the markers).

## W4 · `/overlayRT` decode; remove the built-in display — 2026-07-20

- [ ] **Delete** `ShadowDisplayShader` + the standalone display mesh from the shadow-cast code.
- [ ] Add a **bit-decode overlay mode** to `overlayShader` (float-mod, 5 colours, additive) + route
      `shadow-*` names to it in `overlayModeFor` (this is the `OVERLAY_BITS` mode, re-added for `shadow-*`).
- [ ] List `shadow-a`/`shadow-b` (the current buffer) in `Viewport.renderTextures()` so `/overlayRT
      shadow-a` / `-b` (and `/showRT`) can select them.

## W5 · Verify — 2026-07-20

- [ ] `?focus=100,50&shadowcast` then `/overlayRT shadow-a` (or `-b`): the 5 coloured shadows appear
      **world-aligned**, from in-radius prims (check against the radius-ring markers).
- [ ] **Pan** the camera → the shadows **stick to the world** (stay on their casters' ground), not the
      screen — the whole point of world-space. **Zoom** → they scale with the world (nearest), refilling.
- [ ] Incremental proof still holds: a light moves → only its colour re-casts; the other four persist.
