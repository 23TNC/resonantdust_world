# Todo — shadow-tiered (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. See
[`README.md`](README.md) for the pipeline, [`forks.md`](forks.md), [`issues.md`](issues.md)._

---

**T1–T7 all done + verified — moved to [`completed.md`](completed.md) (2026-07-20).** The 4-RT screen-cast
→ world-merge → combined-display pipeline works: shadows cast per light, coloured, stick to the world, no
aliasing on pan (the `shadow-world` bug is gone), and a moved light re-casts same-frame with no ghost. The
stale-on-pan follow-up ([I-9](issues.md#i-9)) surfaced during verify and was fixed.

Nothing open. The mechanism graduates toward the real [`shadows`](../shadows/README.md) engine; the next
experiment ([`light-data-texture`](../light-data-texture/README.md)) feeds light data from a texture.
