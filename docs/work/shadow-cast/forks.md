# Forks — shadow-cast

_Decision points + options + which we chose + why. All 2026-07-20; most are the user's spec for this
experiment (recorded so the rationale survives)._

---

## F1 · 5 bits in ONE channel (RED), one bit per light — 2026-07-20

5 lights → **bits 0–4 of the RED byte**. One channel deliberately: proves 5 lights in a single byte, and
RGB→24 is the same pattern widened. **A is never used for data** (settled — premultiply-error-prone; see
[`rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md)). Bit `k` = "light
`k` shadows this pixel".

## F2 · Ping-pong on UPDATE, display current between updates — 2026-07-20

The swap (`shadow-a`↔`shadow-b`) happens when a light moves (≈1/second), not every frame: an update reads
the current buffer and writes the other, then displays it; between updates the last-written buffer is just
displayed unchanged. "Send only dirty lights when updating" ⇒ the expensive work is per-update, not
per-frame. (Still a true ping-pong — source ≠ destination each update.)

## F3 · Cast → `mask`, then combine `mask`+`src` → `dest` (avoid the feedback loop) — 2026-07-20

Setting bit `k` where a shadow falls **without disturbing the other bits** needs a read-modify-write of the
bitfield — which can't read the buffer it writes ([I-1](issues.md#i-1)). So split it: **(1)** rasterise the
dirty light's shadows into a separate coverage **`mask`** RT; **(2)** a combine pass reads `src` + `mask` →
writes `dest`. This **mirrors the real pipeline** (`mask` ≈ `shadow-hot`, `shadow-a`/`-b` ≈ `shadow-cold`),
so proving it here proves that seam.

## F4 · Screen-space casting — 2026-07-20

Cast + store + display all in **screen space** (project the lights + in-radius prims world→screen each
update). World-space toroidal storage is already proven by `bitfield-rt` E1–E4 and isn't what this
experiment tests, so keep it simple. (A camera pan would re-dirty all 5 lights — fine; don't pan during
the proof, or accept a full re-cast.)

## F5 · Casters = in-radius standing prims; shadow = a billboard quad — 2026-07-20

Casters are the scene's **standing prims (things) within a light's radius** — re-add the `standingPrims`
enumerator the nuke removed, plus a per-light radius cull. (Or plant a few test caster prims if that's
faster to stand up.) Shadow shape = a **plain billboard quad** projected from the light — the textured
silhouette is deferred ([shadows D-3](../shadows/deviations.md#d-3)); fidelity isn't what's being proven.

## F6 · 5 distinct colours, additive overlap — 2026-07-20

Decode bit `k` → a fixed colour (5 well-separated hues); a pixel sums the colours of its set bits so
**overlapping shadows blend** and you can see which lights shadow where. Same float-mod decode
`bitfield-rt` proved (ES 1.00, no `uint`).
