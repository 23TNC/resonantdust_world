# Completed — shadows on prims

_Delivered + verified. Newest first. Mechanisms in [`README.md`](README.md)._

---

_(none yet — planned. Prior groundwork that this stream builds on:)_

- **Depth channel populated** (`2026-07-23-lighting` #3): `zdepth-world` writes `0x80 | (base_row & 0x7f)`
  for a thing, `0` for ground — the base row this stream reads to derive elevation. Row space matches the
  gather's caster depth (draw-box bottom `prim.y + prim.height`).
- **Binary front/behind** (`2026-07-23-lighting` #3): a thing at/in-front-of the caster takes the
  unshadowed lightmap (`__depthtest`, `uDepthTest`) — the working default this stream improves on. The
  self-shadowing **re-sample scaffold** was tried and **removed** (see [`issues.md#i1`](issues.md#i1)) so
  the blit stays debuggable while pass 2 is built.
