# Completed — binary occlusion + stored caster ids

## 2026-07-30 · P0 — checkpoint

### Design-doc precedence check (before any code)

This stream deletes soft shadows, so `design/` was re-read first to be sure the plan does not
contradict it. **It does not:**

- [`design/shadows.md`](../../components/client/webgl/design/shadows.md) specifies the projected
  **silhouette** model and says plainly: _"Sample the sprite **alpha** as the shadow mask."_ An alpha
  mask is nearer binary than it is to an emitter-coverage integral. No penumbra commitment anywhere in
  the file.
- [`intent/shadows.md`](../../components/client/webgl/intent/) and `intent/tiered-lighting.md` carry no
  soft-shadow requirement either. The single `radius` hit in tiered-lighting is the light data
  texture's field layout (`xy`,`z`,`radius`), not a shadow model.
- The design also sanctions this implementation shape: _"the cold inline sweep tests it per fragment,
  the dynamic tier rasterizes it"_ — so the per-fragment gather is the design's own cold path, not a
  deviation from it.

Recorded because "does the plan contradict design?" is the one question that must be answered before
writing code, and a later reader should not have to re-derive that it was asked.

### The A/B reference

| | |
|---|---|
| **code reference SHA** | **`b35103a3`** — `feat(pawn-part-placement): P2 — per-slot &prim.depth` |
| stream opened at | `e23afbe1` (docs only; code identical to `b35103a3`) |
| `git status` at recording | **clean** |

No checkpoint commit was needed: the working tree was already clean when the stream opened, and the
only commit since is the plan itself, which touches no code. `b35103a3` is therefore the exact code
state every "before" number in this stream is measured against, and it is reachable from HEAD.

Worth noting for anyone reconstructing history: the `span 1` edit to `content/visual/pawns.rd` made
earlier in this session is **in** that reference — another session swept it into `b35103a3` rather than
it being lost.

### The penumbra histogram — F1 settled, and it validates the whole stream

Read back both shadow RTs (`debugReadShadow(0)`/`(1)`), unpacked every slot byte as
`u7 coverage | u1 on-billboard`, histogrammed the coverage. area1, zoom 1, 3 content torches.

| | cold | hot |
|---|---|---|
| occupied slot-samples | 85 629 | 70 873 |
| **saturated (cov = 127)** | **31 155** | 231 |
| **partial (cov 1–126)** | **0** | **20** |
| cov 0, flag bit only | 54 474 | 70 622 |
| **% partial** | **0.0 %** | **0.03 %** |

**The cold shadow map is perfectly bimodal. Not one texel in 31 155 carries a partial value.**

The `cov = 0` population is not shadow — it is the `on-billboard` flag set with zero coverage, i.e.
"this texel sits on a billboard". So the *actual* cold shadow population is 31 155 samples and **every
single one is fully saturated**.

**This reframes the stream.** P1 is not sacrificing soft shadows for speed — the stored value already
has exactly two states, and the analytic λ interval, the chord→area curve and the `frac` arithmetic are
computing a continuous quantity that collapses to binary before it is ever written. Binary occlusion is
not an approximation of today's behaviour; **it is an exact description of it.**

The user was right and my "penumbra is live" claim was wrong twice over — wrong from the screen (their
screenshot) and now wrong from the data.

**What this does NOT establish** is *why* it collapses. The geometry predicts ~21 texels of gradient for
a conifer at `radius 0.35`, and an earlier histogram in this session at `radius 2.0` found 61 % partial —
so the machinery demonstrably can produce partial values, at a large enough emitter. The most likely
mechanism is the λ-interval clamp: when `Dslope` is small relative to the card width, both roots land far
outside `[-1,1]`, the clamp returns the full range or nothing, and `frac` is 1 or 0. At `radius 0.35`
that appears to be every texel.

Filed as [I4](issues.md) rather than chased: it does not change what this stream builds, and the answer
would only matter to someone trying to *restore* soft shadows later — for whom "it was clamping, not
missing" is the useful sentence.

### The light-scaling harness

N orbiting lights on distinct tiles (`PRES_SLOTS` caps 16 per tile, so clustering would silently drop
lights), each authored to match the content torch — `emitterRadius 0.35`, `height 2.5` tiles,
`castShadows`, cold class. 90 frames after 12 warm-up, tick-driven, cold gather + cold lighting timed by
render target.

**Calibration passes on both dials**, which is the acceptance:

| | gather ms | dirty tiles |
|---|---|---|
| N1 reach 8 | 0.166 | 39 |
| N1 reach 16 | **0.610** (3.7×) | 263 |
| N2 reach 16 | 1.279 | 370 |
| N4 reach 16 | 2.100 | **512** (window full) |

Dirty saturates at 512 — the whole window — from N4 at reach 16 onward. Past that, added lights cost
per-tile-per-light work rather than more tiles, which is the regime the headline measures in.

### THE HEADLINE, before — 15 moving lights

Reach 16, zoom 1, 3 repeats, gather + lighting:

| N | mean ms | spread | |
|---|---|---|---|
| 12 | 7.44 | 0.26 | |
| 13 | 6.96 | 0.45 | |
| 14 | 7.44 | 0.48 | |
| **15** | **7.83** | 0.36 | **fits** |
| **16** | **8.30** | 0.54 | **over** |

**Answer: 15.** The boundary is 7.83 → 8.30 ms.

Two honest qualifiers. The data is **non-monotonic** (N13 measured below N12) and the spread reaches
0.54 ms, so 15 is the crossing of a noisy trend, not a sharp edge — anything from 14 to 16 is defensible
and 15 is the best single answer. And **gather is 87 % of it**: at N16, 7.56 ms gather against 1.03 ms
lighting. Every optimisation this stream plans targets the gather, which is the right place.

### The supporting matrix (single repeat; the headline above carries the repeats)

`gather · lighting · total` ms, zoom 1:

| | N1 | N2 | N4 | N8 | N16 |
|---|---|---|---|---|---|
| **reach 4** | 0.14 · 0.10 · **0.24** | 0.19 · 0.11 · **0.30** | 0.25 · 0.23 · **0.47** | 0.35 · 0.24 · **0.59** | 0.37 · 0.18 · **0.55** |
| **reach 8** | 0.24 · 0.11 · **0.34** | 0.49 · 0.27 · **0.76** | 0.57 · 0.25 · **0.82** | 1.13 · 0.35 · **1.48** | 1.55 · 0.38 · **1.92** |
| **reach 12** | 0.52 · 0.17 · **0.70** | 0.70 · 0.29 · **0.99** | 1.86 · 0.50 · **2.37** | 2.22 · 0.49 · **2.71** | 2.99 · 0.55 · **3.54** |
| **reach 16** | 0.61 · 0.29 · **0.90** | 1.28 · 0.35 · **1.63** | 2.69 · 0.52 · **3.21** | 3.90 · 0.71 · **4.61** | 6.21 · 0.93 · **7.14** |

Reach dominates far more than N: 16 lights at reach 4 (0.55 ms) is cheaper than **one** at reach 16
(0.90 ms). Single-repeat noise is real — `r16_N16` reads 7.14 here against the 3-repeat mean of 8.30.

### Before-image

Reproducible recipe for the P5 comparison: `?user=Claude&focus=102,57&zoom=2`, crop region
`[700, 20, 1100, 300]`, content scene (3 torches), fully streamed. The user's own zoom-2 screenshot of a
conifer edge earlier in the session is the sharper reference for what the stepped edge looks like; this
recipe is the one to re-shoot after P4.

## 2026-07-30 · P1 — binary occlusion

### What was deleted

The emitter-interval penumbra, in **both** copies — the main card in `casterCover` and the
perpendicular card in `casterCoverNS`. Each solved `u(lambda) = u0 + lambda*Dslope` at the card's two
edges, clamped to `[-1,1]`, took the span as `frac`, and scaled the silhouette by it. Both are now:

```
if (u0 < 0.0 || u0 > 1.0) return 0.0;   // centre ray misses the card -> lit
```

`max(cov, cc)` became **`any`** — `cov = 1.0; break;` — with `if (cov > 0.0) break;` unwinding the
bucket, tile and DDA loops. Those are **body statements, never loop conditions**: a body-modified
variable in a `for`-condition is the documented miscompile trap in this file.

### The new slot byte (v4), defined in one place

| bit | meaning |
|---|---|
| 0 | on-billboard flag (unchanged) |
| 1 | **OCCLUDED** (was `u7` coverage in bits 1–7) |
| 2–7 | **reserved, written zero** |

Every reader was moved off `((byte) >> 1u) / 127.0` to `(lane >> (shK + 1u)) & 1u` — the two bilinear
taps in `accumulateLights`, the cold-delta taps, the overlay, and the flicker splat's shadow stamp.

### Verified

| check | result |
|---|---|
| distinct byte values, cold | **3** — `1` (flag only), `2` (occluded), `3` (both) |
| **reserved bits 2–7 set anywhere** | **0** |
| corridor↔brute identity | **0 differing** of 22 865 non-zero |
| shaders compile, scene renders | yes — art, shadows and lighting all correct at area1 |

### Measured — same fixture as P0 (`focus=100,50`, zoom 1, reach 16, 3 repeats)

| N | P0 total | **P1 total** | gather P0 → P1 | delta |
|---|---|---|---|---|
| 12 | 7.44 | **6.08** | — | −18 % |
| 14 | 7.44 | **5.97** | — | −20 % |
| 15 | 7.83 | **6.22** | — | −21 % |
| **16** | **8.30** | **7.11** | 7.56 → **6.19** | **−14 %** |

**The headline moves 15 → 16 lights**, and 16 is the `PRES_SLOTS` ceiling — exactly the saturation
[I5](issues.md) predicted. So from here the number to watch is **headroom at N16**, which is
**8.30 → 7.11 ms, 1.19 ms freed**, with 0.89 ms of the 8 ms budget still unspent.

**Attribution.** This is deleted arithmetic plus the `any` early exit; no geometry changed, which the
0-differing identity confirms. The early exit is doing real work — the gather cannot know it is finished
until an occluder is found, and now it stops there instead of testing every remaining caster in the
corridor.

### A trap worth recording

`npx tsc --noEmit` **passed on a genuinely broken shader**. GLSL lives inside a template literal, so TS
type-checks it as a string and cannot see unbalanced parens or a comment that swallowed a statement —
both of which a careless bulk `str.replace` introduced here. Only loading the page catches it. Two
guards followed from that: edit shader text with the Edit tool (the backtick hook fires; a heredoc
bypasses it — and a backtick in a GLSL comment closed the template literal on the first attempt), and
**never** treat a green typecheck as evidence a shader is valid.
