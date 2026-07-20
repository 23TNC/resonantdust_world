# Todo — light-data-texture (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. Extends `shadow-tiered`'s
`shadowCast.ts` + `shadowCastShaders.ts`. See [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

**L1–L4 all done + verified — moved to [`completed.md`](completed.md) (2026-07-20).** The `RGBA32F` 5×5
data texture feeds per-light colour to the display shader; rewriting it every frame makes the shadows
flicker through colours per-light, proving the live JS→texture→shader path. The float path worked (u8
fallback not needed). L5 (position from the texture) is deferred as an optional stretch — see completed.

Nothing open. Mechanism graduates toward the real [`shadows`](../shadows/README.md) engine.
