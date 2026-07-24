# Completed — shadows on prims

_Delivered + verified. Newest first. Mechanisms in [`README.md`](README.md)._

---

## P0–P3 · Shadows climb billboards — DELIVERED + verified 2026-07-24
Built as ONE fragment shader (not two draws): the gather branches per texel on the zdepth-world sample.
- **P0 elevation + depth plumbing.** `GATHER_FRAG` binds `uZDepth` (the `zdepth-world-cold` composite) and
  samples it BY WORLD POSITION — the composite is a padded slot-atlas, so the UV is
  `((slot+1)·slotPx + frac·slotPx) / texW` on the SAME toroidal window as shadow-cold (`uZSlot`/`uZTexW`/
  `uZTexH` uniforms; slot pitch comes from `SquareCache.window.slotPx`, now on `TileWindow`). Per texel:
  is-thing + base row → `zElev = uElevK · max(0, base_row·UPT − P.y)` (units). `uElevK` defaults to
  `sin65` (the ratified model); `__elevk` tunes the climb by eye ([forks F5](forks.md#f5)).
- **P0b + P1 the CONE, one map.** Ground texels keep the lifted ground point (unchanged ⟹ P6 identity
  preserved by construction). Thing texels project to `G = L.xy + s·(P − L.xy)`, `s = Lz/(Lz − zElev)` —
  the elevated pixel's ground shadow, which CLIMBS + tapers (high pixels project past the caster's finite
  ground shadow ⟹ lit). `casterOne()` (shared by the corridor + brute walks) adds the two thing-only culls:
  (1) self / same-row exclusion (kills self-shadow), (2) front/behind `dot(Cb−Rbase, Rbase−L) < 0` — the
  caster must sit between the light and the receiver ([forks F1](forks.md#f1), [F7](forks.md#f7),
  [F8](forks.md#f8)). The cull also bounds every valid caster inside the light's reach, so corridor + brute
  both visit it.
- **P2 consume, no composite.** The one `shadow-cold` already holds ground-on-ground + prim-on-prim
  (presence-partitioned). The blit consumes the shadowed irradiance directly on thing texels (`uPrimShadow`,
  default on); the OLD binary front-thing restore stays behind `uPrimShadow == 0` / `__primshadow(false)` as
  an A/B baseline ([forks F3](forks.md#f3)).
- **P3 cold/hot + dirty — free.** One gather runs per class already, so pass 2 inherits the cold/hot split
  and the union dirty with no new machinery ([forks F4](forks.md#f4)).

**Verified in-browser** (single 12-tile light over the forest, `?focus=34,27&zoom=2.5`, all lights frozen
static + a zero-reach keep-alive light so the change-gated render loop keeps ticking):
- **corridor ↔ brute BIT-IDENTICAL with the prim path live** — 29 456 non-zero shadow words, **0
  mismatches** (`__corridor` + `debugReadShadow(0)` diff). The shared `casterOne` + the reach-bounding cull
  earned the P6 identity back for pass 2.
- **climb / flat / binary all distinct + correct**: `elevk sin65` climbs + tapers; `elevk 0` paints the
  ground shadow flat over the billboard (the old "flat overlay" symptom); `__primshadow(false)` restores the
  whole front-thing to lit (the old binary). No self-shadow on trees/bushes; ground shadows unchanged; no
  console/GLSL errors (the Programs built ⟹ the shaders compiled).

**Hand-off (P4, small):** final `elevk` eye-tune (sin65 is the principled default, live-tunable); retire the
binary path once the climb is trusted (kept as the A/B for now); other-caster height residual
([forks F2](forks.md#f2)) still a later refinement. The render loop being change-gated (idles when fully
static) is a debug gotcha, not a bug — freeze with a zero-reach keep-alive light to inspect a static frame.

---

_Prior groundwork that this stream builds on:_

- **Depth channel populated** (`2026-07-23-lighting` #3): `zdepth-world` writes `0x80 | (base_row & 0x7f)`
  for a thing, `0` for ground — the base row this stream reads to derive elevation. Row space matches the
  gather's caster depth (draw-box bottom `prim.y + prim.height`).
- **Binary front/behind** (`2026-07-23-lighting` #3): a thing at/in-front-of the caster takes the
  unshadowed lightmap (`__depthtest`, `uDepthTest`) — the working default this stream improves on. The
  self-shadowing **re-sample scaffold** was tried and **removed** (see [`issues.md#i1`](issues.md#i1)) so
  the blit stays debuggable while pass 2 is built.
