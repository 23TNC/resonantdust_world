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

### Where to resume (P2), and the harness recipe

The tree is clean and P1 is committed, so P2 starts from a verified checkpoint. To rebuild the
measurement rig in a fresh session:

1. Serve at **`?user=Claude&focus=100,50&zoom=1`** — the P0/P1 fixture. A different focus is a
   different caster density and the numbers stop being comparable (this was nearly got wrong once).
2. Wait for the corpus to stream (~15 s); a geo-tier frame measures the wrong thing.
3. Wrap `tick` to capture `__lastArgs`, then install `__setlights(n, reachTiles)` /`__hzPump(90)` /
   `__hzDrain()` — full source is in this session's transcript; they place N lights on DISTINCT tiles
   (clustering silently drops past `PRES_SLOTS`), 12 warm-up frames dropped, timed by render target.
4. `__orbit(true)`, then 3 repeats at N 12/14/15/16, reach 16.

**Compare against:** N16 = **7.11 ms** (gather 6.19, light 0.92). That is the number P2–P4 have to beat,
and the budget left is 0.89 ms before 8 ms.

### Post-revert verification (fresh Chrome, after the I6 lock-up)

Reloaded the reverted P1 build in a new browser instance and re-checked every P1 acceptance:

| check | result |
|---|---|
| renders | correct — trees, shadows, lighting, the wolf and the human pawn all present |
| `isContextLost` / `getError` | **false / 0** |
| `coldShadowRT` attachments | **1** — the P2 MRT change is fully gone |
| slot byte values | `1` / `2` / `3` only; **reserved bits 2–7 set: 0** |
| corridor↔brute identity | **0 differing** of 31 526 non-zero |
| N16, reach 16 | **6.36 ms** (gather 5.52, light 0.83) |

**6.36 ms against the 7.11 ms recorded pre-lock-up.** Absolute timings shift between browser
instances — fresh GPU state, no accumulated context — so the two are not directly comparable and 7.11
stays the recorded P1 figure. What matters is unchanged and if anything stronger: **16 lights at reach
16 fit inside 8 ms**, where the P0 baseline was 8.30 ms and did not.

The lock-up was entirely the P2 MRT attempt. P1 is unaffected.

## 2026-07-31 · P2 (part 1) — 8 lights per tile ([F7](forks.md#f7))

`PRES_SLOTS = TILE_SLOTS` (8). The hi presence set is retired: one `fetchLin` instead of two in all four
shaders that read presence, the light loop is `slot < 8`, and `writePresence` writes one set. Eviction
still clears the hi band so a stale tile cannot present phantom lights.

### Verified

| check | result |
|---|---|
| renders, context alive | yes — `isContextLost` false, `getError` 0 |
| **slots 8–15 occupied** | **0** (slots 0–7: 43 074) — the upper half is genuinely gone |
| corridor↔brute identity | **0 differing** of 32 913 |

### Measured (same fixture, reach 16)

| | P1 (16 slots) | **P2a (8 slots)** |
|---|---|---|
| N8 | — | **4.19 ms** |
| N16 | 6.36 ms | **5.65 ms** |

**The N16 comparison is NOT like-for-like and must not be quoted as a pure speedup.** At reach 16 every
light covers the whole window, so every tile wants all 16 and can now hold only 8 — roughly half the
light–tile pairs are dropped. Some of that 6.36 → 5.65 is work not done rather than work done faster.

The honest number for the new configuration is **N8 at 4.19 ms**, where every light is fully
represented on every tile it reaches. Against the P0 baseline's 8 ms budget that is a very large margin —
but it is a *different capability*, and P5 must present it that way rather than as 16 lights got cheaper.

### Still to do in P2

The encoding change itself: the texel currently still holds the P1 coverage byte, not `8 × u16` caster
ids. The slot-count reduction is the prerequisite that makes it fit; the re-encode, the id write and the
sentinel are the remaining three items.

## 2026-07-31 · P2 COMPLETE — the texel is the id map

### The v5 encoding

The shadow texel stops holding a value and holds the **caster**. 8 slots x u16 = 128 bits = the same
`uvec4`, laid out exactly like `tileSlot` (slot i in lane `i>>1`, high half when `(i&1)==0`):

```
bit 15     on-billboard flag
bits 0-14  the occluding caster's billboardIdx, or SHADOW_NONE (0x7fff)
```

**Occlusion is no longer stored, it is implied** — a slot is shadowed iff its id is not the sentinel. No
coverage bit exists to drift out of step with the id. Decoded through one helper (`shadowOcc` /
`shadowCaster`) defined beside `tileSlot`, with local copies in the two shaders that deliberately do not
pull in `GATHER_COMMON` (overlay, splat).

`walkShadow` gained an `out uint winner`, set where the walk stops. Under binary any-semantics that is
exactly the caster that occluded — there is no ambiguity about which of several "won".

### Verified

| check | result |
|---|---|
| renders | shadows correct at area1 — trees, wolf, human pawn |
| occluded samples carrying an id | **24 561** |
| `no caster` slots | **1 024 015** — and 24 561 + 1 024 015 = 1 048 576 = 512x256x8 **exactly** |
| **distinct caster ids** | **80** |
| ids vs allocation | observed range ~200-450 against **459 billboards allocated** — every id is a real caster |
| on-billboard flag preserved | 21 433 slots flagged |
| corridor↔brute identity | **0 differing** |
| `getError` / `isContextLost` | 0 / false |

The slot-count arithmetic closing exactly (1 048 576) is the strongest single check: it proves every
slot is either a real id or the sentinel, with nothing uninitialised.

### Measured (reach 16) — the id map is free

| | P2a (8 slots, coverage byte) | **P2 (8 slots, id map)** |
|---|---|---|
| N8 | 4.19 | **4.07** |
| N16 | 5.65 | **6.08** |

Within run-to-run noise in both directions. Storing a 15-bit id costs no more than storing a 1-bit
value, which is the result that matters: **the identity was available for free all along, and the two
lock-ups came from trying to store it somewhere other than where the value already lived.**

15 bits holds 32 767 casters against 459 allocated — a 70x margin, so the sentinel choice is not tight.

### The bug worth keeping

First working build rendered **no shadows at all**. Cause: I seeded the lanes to the sentinel
(`0x7fff7fff`) and then wrote with `|=`. `0x7fff | id == 0x7fff` for every id <= 0x7fff, so every slot
read back as "no caster". Fixed by clearing the slot's half before OR-ing.

Seeding to zero instead would have been worse and quieter: **billboardIdx 0 is a real caster**, so an
unwritten slot would have read as "caster 0 occludes here" — a plausible-looking wrong shadow rather
than an obviously missing one.

## 2026-07-31 · F8 implementation — scouted, not started

Located every site F8 touches before running out of room to do it safely. Recorded so the next session
starts from findings rather than a search.

**The `child` pattern already exists** — `childPrimUnder` (`coldShadowData.ts:765`) documents it for
`prim_data`: _"`child = 1` makes RED's top half the `parent_id`"_, with bias-8 signed offsets otherwise.
So F8's parentless-billboard reuse is **extending an established convention, not inventing one**, which
is the right shape and should read as familiar to anyone who knows `prim_data`.

**`light_data` already carries a resolved zone** — its A word is
`u12 reach | u8 emitter_radius | u8 resolved_zone (4-11) | u4 reserved`. So "the record carries enough to
self-position" is precedent in this very band family, not a new idea.

**The absolute address already has an encoder** — `encodePosition(x, y)` builds the carrier prim's R
word, and per `VARIABLES.md` the full form is `region | zone | tile | anchor`. F8's `u8 region | u8 zone`
is the top half of exactly that, so the CPU side is a re-slice of a value already computed, not a new
derivation.

**Sites to change, in dependency order:**

1. `VARIABLES.md` billboard_data R + A — it OWNS this layout, so it changes FIRST (repo convention:
   `VARIABLES.md` outranks code).
2. `coldShadowData.ts` — the billboard writer: emit `u8 region | u8 zone` into R's top half when the
   billboard has no parent, and set the new `u1 child` bit in A's `u14 reserved (0-13)`.
3. `shadowGather.ts` `resolvedTilePos` — take the high bits from the record when `child == 0` instead of
   wrapping against a caller-supplied `ref`; `casterOne`'s `ref` parameter then falls away for
   parentless casters, which is what makes an id alone sufficient.
4. Shadow RT to 2 px per texel (header + ids) — single attachment, 2x width, NOT MRT.
5. `GATHER_FRAG` — write the header px (4 channels x 2 lights x 4 x u4 set) and the id px
   (8 x u16 in-set id).
6. Readers — `accumulateLights`, the overlay, `debugReadShadow`, the identity diff.

**Nothing was changed.** Tree clean at the F8 planning commit.

## 2026-07-31 · B2 dissolved by F8 {#2026-07-31--b2-dissolved-by-f8}

B2 asked the user to choose between reopening the storage question and closing the stream on P1+P2. They
did neither — they showed that **both premises were wrong**, and [F8](forks.md#f8) records the answer.

**B2's premise.** [I7](issues.md): a caster's position decodes only mod 16 tiles against a caller-supplied
`ref`, so an incumbent recovered from the id map cannot be positioned, and the failure mode is a false
positive — a phantom shadow. B2 concluded the fix was ~25 bits per slot and therefore a second texel,
i.e. the storage decision [F7](forks.md#f7) had just closed.

**Why the premise did not hold.**

| B2 assumed | F8 established |
|---|---|
| The record cannot self-position, so a `ref` is unavoidable | **Parentless billboards were never handled.** `parent_id` carries nothing for them, so it is reused as `u8 region \| u8 zone`, gated by a new `u1 child` bit. Those are precisely the HIGH position bits the record lacked — so it self-positions and `ref` falls away. |
| A caster reference is `u16` | It is **`u20`** (`u4 set \| u16 in-set id`) — the data texture is 16 u16-addressable sets. So F7's 128-bit arithmetic never closed either, for a reason neither F7 nor B2 knew. |

**What replaces the "second texel" trade.** Not more bits per slot, but a **header px**: 4 channels x 2
lights x `u4 set`, plus one id px per caster. The shadow texel becomes 2 px at N=1, growing to 5 px at
N=4 — which turns N into a dial rather than a rewrite, the thing the user asked for from the start.

**Status.** Dissolved as a blocker: nothing here needs the user's input any more. The remaining work is
implementation, scouted above and unstarted. P3's early-out and P4's fine placement rest on it.

**Recommendation withdrawn.** B2 recommended closing the stream on P1+P2 and re-planning storage as its
own stream. That was reasoning from inside a constraint I had not checked was real — the same pattern the
user corrected four times before it. P1+P2 remain independently valuable and committed, but they are not
the stopping point B2 argued they were.

## 2026-07-31 · P2b — a billboard root positions itself (F8, first five items)

**`VARIABLES.md` first**, since it owns the layout. `billboard_data.R` gains the same ROOT/CHILD split
`prim_data` has had all along, gated by a new `u1 child` at `A` bit 13:

```
R  ROOT  (child=0)   u8 region | u8 zone | u8 resolved_tile | u8 resolved_unit
   CHILD (child=1)   u16 parent_id      | u8 resolved_tile | u8 resolved_unit
```

**The 16 bits were dead.** Every billboard already got a private carrier prim purely so it had somewhere
to be placed, and no GPU consumer has ever read a billboard's `parent_id` — the resolve happens CPU-side
and the record is stamped with the answer. Those bits are exactly the width of the two HIGH position
bytes, so the trade cost nothing.

**The CPU side was a re-slice, not a derivation.** `resolveCarried` already returns
`encodePosition(...)` — a full `region|zone|tile|unit`. The old writer took that value and *threw the
top half away*:

```
- ((prim & 0xffff) << 16) | (((r.pos >>> 8) & 0xff) << 8) | (r.pos & 0xff)
+ r.pos >>> 0
```

**The shader side collapsed to one helper.** Four call sites repeated the same two-line decode; they now
all route through `billboardPos(Pd, ref)`, so the ROOT/CHILD split exists in exactly one place. A root
takes `decodePos(Pd.x)` — **absolute, no reference, no period, exact at any separation**. `decodePos`
already existed in the shader (it is what `prim_data` roots use), so [I7](issues.md) died on a
substitution rather than the ~25-bits-per-slot second texel [B2](blockers.md) was asking the user to
authorise.

### Verified

- **Corridor↔brute identity: 0 differing of 524 288 texels**, on BOTH classes (cold and hot),
  `__corridor` toggled with the orbit frozen. This item changes only how a position is recovered, so
  identity is the right check: any decode error would move a caster and change the map.
- **Both record paths, read back from the live mirror**: 458 live billboards — **457 roots, 1 child**.
  The one child is billboard 2 with parent 1, i.e. the pawn's head carried on its body, which is the
  only composite in the scene. Root 1 decodes to world tile **100,50** — the camera's focus tile.
- **The page loads and renders.** tsc cannot see GLSL, so a page load is the only real gate.

### A false alarm worth recording

The first load after these edits showed `compile shadow-gather.frag: 'win' : undeclared identifier` at
line 778 — alarming, since `win` sits in `GATHER_FRAG` which this work never touched. It was **stale
console history**: the reader returns messages captured since the tab opened, and both reads carried the
identical `9:25:13` timestamp. Clearing the console and reloading at the P2 commit showed no error, and
reloading again with F8 restored showed no error either.

What settled it before the browser did: reassembling `GATHER_FRAG` offline put `win` at line **800**,
not 778 — the error came from a source 22 lines shorter, i.e. a mid-edit HMR compile. Worth keeping: a
console read here is not a fresh observation unless it is cleared first, and shader line numbers are
precise enough to date the build that produced them.
