# Completed — shadow-world

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## W1–W5 · DONE + verified in-browser 2026-07-20

`?focus=100,50&shadowcast&overlayRT=shadow-a`: the 5 cold lights cast billboard shadows onto the ground
in **world coords** into a **world-space ping-pong bitfield** (`shadow-a`/`shadow-b`), which lives in the
**same toroidal buffer layout as `albedo-cold` et al.**; `/overlayRT shadow-a` decodes its RED-byte bits
to 5 colours, world-aligned. **Both new things verified:**

- **World-space storage** — dragging to pan shifted the terrain *and* the shadows together; the colours
  stayed clustered on their light markers / casters (they stick to the world, not the screen). This is
  the payoff over shadow-cast's screen-space store.
- **Display via `/overlayRT`** — the built-in colour display is gone; the bit→colour decode is an overlay
  mode (`OVERLAY_BITS`, routed for `shadow-*`), so `shadow-a`/`shadow-b` are inspected like every other
  G-buffer channel.

Everything from shadow-cast carried over (5 lights / 5 bits / incremental per-light updates / the light
dot + radius-ring markers). Incremental + carry-forward still hold (multiple lights' colours coexist).

**How it maps to the buffer:** a world px `(wx,wy)` → buffer px `(mod(wx/SQUARE, cols)+1)·slotPx` — the
same toroidal mapping the composites use (`SquareCache.bufferMapping()` exposes it). A shadow quad crossing
the buffer seam is drawn at **±period copies** so it wraps ([F3](forks.md#f3)). The combine is a full-buffer
per-texel RMW (clear bit k, set from mask, carry the rest); zoom/resize clears + re-casts (a bitfield can't
bilinear-reproject).

Implementation: edited `shadowCast.ts` (world-space RTs sharing the cache buffer, world-coord cast +
toroidal wrap, combine, markers), removed `ShadowDisplayShader` from `shadowCastShaders.ts`, re-added
`OVERLAY_BITS` to `overlayShader.ts` (routed for `shadow-*`), and wired `shadow-a`/`shadow-b` into
`Viewport.renderTextures`/`overlayComposite` + `SquareCache.bufferMapping()`. Toggled by `/shadowcast`.

**Conclusion:** this is a working **world-space `shadow-cold`** — the last big piece the real
[`shadows`](../shadows/README.md) engine needs. Known limits (fine for the experiment): large pans can
leave toroidal-seam staleness until the next re-cast; zoom clears + refills.

