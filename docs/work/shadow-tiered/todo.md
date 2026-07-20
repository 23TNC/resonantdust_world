# Todo — shadow-tiered (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Edits the
`shadow-world` code. See [`README.md`](README.md) for the pipeline, [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

## T1 · Expose the window origin — 2026-07-20

- [ ] Add `winCol`/`winRow` (the resident window's world-square origin) to `SquareCache.bufferMapping()`
      — the piece `shadow-world` lacked, needed for the screen→buffer 4-rect placement ([I-5](issues.md#i-5)).

## T2 · The four RTs + light groups — 2026-07-20

- [ ] `screen-shadow-a`/`-b` — screen-space bitfield RTs (viewport-sized, `nearest`, A=1), ping-pong.
- [ ] `shadow-a`/`-b` — toroidal world-space bitfield RTs (fixed-buffer size, share the cache mapping),
      ping-pong. `/overlayRT shadow-a` samples the CURRENT one, world-aligned.
- [ ] `l-a`/`l-b` — the lights refreshed on a-/b-frames (the realtime set). Drive from the moved lights,
      or a rotating subset so the world tier is exercised, not degenerate ([F5](forks.md#f5), [I-3](issues.md#i-3)).

## T3 · Cast the realtime set in SCREEN space — 2026-07-20

- [ ] For each light in the frame's realtime set, project its in-radius prims' billboard shadows to the
      ground, map **world→screen** (as `shadow-cast` did — window-bounded, no aliasing), rasterise into
      `screen-shadow-a` (each light → its bit; `max`/OR union). Clear `screen-shadow-a` first.

## T4 · Remove pass: `shadow-a = shadow-b` with the realtime bits cleared — 2026-07-20

- [ ] A full-buffer shader pass reads `shadow-b`, **clears each realtime light's bit** (per-bit
      `n -= set?2^i:0`, float-mod — [I-2](issues.md#i-2)), writes `shadow-a`. Read `-b`, write `-a`
      (ping-pong, no feedback). This drops the moved lights' *old* shadows so they don't ghost.

## T5 · Add pass: bake last frame's realtime (`screen-shadow-b`) into `shadow-a` — 2026-07-20

- [ ] Map `screen-shadow-b` (screen) into the buffer → up to **4 wrapped rects** (using `winCol`/`winRow`),
      and **additive-blend** them onto `shadow-a` (`src + dst`, fixed-function — NOT a shader read of
      `shadow-a`, so no feedback loop; [I-7](issues.md#i-7)). Exact-OR because bits are disjoint
      ([F3](forks.md#f3), [I-1](issues.md#i-1)). (Clear `l-a` from `screen-shadow-b` first only if the
      realtime sets can overlap.)

## T6 · Display: `shadow-a` OR `screen-shadow-a` — 2026-07-20

- [ ] Decode both bitfields to colours and OR them: sample `shadow-a` at the **world-aligned** UV (the
      `/overlayRT` path) and `screen-shadow-a` at the **screen** coord (`gl_FragCoord`), combine
      ([I-4](issues.md#i-4)). Expose `screen-shadow`/`shadow` to `/overlayRT` for inspection.

## T7 · Verify — 2026-07-20

- [ ] `?focus=100,50&shadowcast` + `/overlayRT shadow-a`: shadows cast from in-radius prims, coloured per
      light. **Pan far** → shadows stay in the (100,50) zone only, **no aliased copies** in other zones
      (the `shadow-world` bug, fixed). Shadows stick to the world.
- [ ] **Move a light** → its old shadow **clears** (no ghost — the remove pass), new one appears same
      frame (realtime screen cast). The other lights are undisturbed (carry-forward).
