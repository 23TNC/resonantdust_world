# Deviations — shadows

_Where this foundation departs from the durable design
([`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md),
[`design/shadows.md`](../../components/client/pixijs/design/shadows.md)). Pre-logged because they are
**deliberate**, not accidents. D-1..D-4 are count-shrinks (each names its un-shrink phase). **D-5 is a
genuine architecture divergence** — the design bakes cold per-rect in world-space; this stream casts in
screen-space and copies to world-space, specifically because the per-rect world-space bake is what sank
the nuked attempt. Format: what the design says → what this stream does → why → un-shrink/status._

---

## D-5 · Screen-space cast + copy-to-world, NOT a per-rect world-space bake — 2026-07-19

**Design** ([tiered-lighting §Cold](../../components/client/pixijs/intent/tiered-lighting.md)):
`shadow-cold` is **baked per-rect in world-space** (on dirty), rect by rect, via the scatter engine.

**This stream:** casts shadows **once, in screen space** (`shadow-hot`, whole viewport, every frame), then
**copies** that into the world-space `shadow-cold` bitfield with the 4-way toroidal wrap
([F8](forks.md#f8)–[F10](forks.md#f10)).

**Why (the load-bearing reason):** the per-rect world-space bake forced **partial-shadow splitting**
(a shadow crossing rect boundaries drawn into each) and **which-rect-when-a-light-moves** re-dirtying —
the two problems the nuked build fought and lost. Casting in screen space erases both: one surface, no
boundaries, and a moved light just re-casts next frame. This is not a shrink of the design — it's a
**better mechanism for the same result**, and it may well *replace* the per-rect bake in the design once
proven. **Status:** if it holds up, promote it into `intent/tiered-lighting.md`; the true "cold = baked
static" tier (for hundreds of never-moving authored lights) can layer on later as an optimisation, not a
prerequisite. Related: [D-6](#d-6).

## D-6 · "cold" here is round-robin (warm behaviour) — provisional name — 2026-07-19

**Design:** *cold* = static, baked-on-dirty; *warm* = dynamic, round-robin-refreshed at display.

**This stream:** the single bitfield is called `shadow-cold` but is **refreshed round-robin** (3 lights/
frame, ~8-frame cycle — [F12](forks.md#f12)), which is *warm* behaviour.

**Why:** the foundation needs one bitfield, and round-robin is the general case (a static light is just one
that never changes between refreshes). Naming it "cold" now keeps continuity with the RT name; the true
cold/warm split is a later concern. **Un-shrink:** "we'll swap cold → warm later" — rename and, if worth
it, add a separately-baked static-cold tier.

## D-1 · Bitfield is a RED byte (6 bits) now → RGB (24) goal, not 32-bit — 2026-07-19

**Design** ([tiered-lighting §Cold](../../components/client/pixijs/intent/tiered-lighting.md)):
`shadow-cold` is a **32-bit** bitfield across the full RGBA texel.

**This stream:** **RED byte** (6 bits) this iteration; **RGB (24 bits)** goal — **A held at 1**.

**Why:** 6 lights fit one byte; a single channel keeps pack/decode minimal to bring up. RGB is the same
shader over 3 bytes. **Not 32:** **A never works** as a data lane on the premultiply paths
([F2](forks.md#f2)), so the ceiling is 24. **Un-shrink:** pack across G/B (RED→RGB, 6→24).

## D-2 · `shadow-hot` is 3 screen-space RGB lanes, not 8-lane `uChannel` scatter maps — 2026-07-19

**Design** ([tiered-lighting §engine](../../components/client/pixijs/intent/tiered-lighting.md)): the
scatter engine writes **8 lanes** (2 RGBA maps × 4) via the `outColor = uChannel` trick.

**This stream:** one **screen-space** RT, **RGB = 3 lanes**, A free.

**Why:** 3 plain colour lanes need no `uChannel`/premultiply subtlety and pair 1:1 with the byte's batches
of 3. **Un-shrink:** reclaim more lanes if the throughput target (3/frame) needs raising.

## D-3 · Casters are solid billboard quads, not textured silhouettes — 2026-07-19

**Design** ([design/shadows.md](../../components/client/pixijs/design/shadows.md)): a **5-triangle fan**
of the billboard silhouette with per-corner depth, sampling sprite **alpha** via per-triangle UVs (+ the
`outline` earcut).

**This stream:** the **4 billboard corners** → a **solid 2-triangle quad**. No fan, depth, UV, alpha, or
`outline`.

**Why:** the cast→copy→pack→decode pipeline is what's being proven; the silhouette is a fragment-level
refinement that layers on without touching it. A blocky rectangular shadow is the correct foundation
output. **Un-shrink:** swap the solid quad for the 5-tri fan + presence depths + UV-alpha of `design/shadows.md`.

## D-4 · Lights are 6→24 debug uniforms, not a per-rect light-data texture — 2026-07-19

**Design:** each rect reads its nearest 32 cold lights from a **per-rect light-data texture**.

**This stream:** **6 (→24) debug lights** in a plain array ([F6](forks.md#f6)). (And since casting is
screen-space + global, there's no per-rect light set to texture anyway — the per-rect texture is a
cold-bake concept that D-5 sidesteps for now.)

**Why:** no content source of cold lights yet, and 24 is the whole target. **Un-shrink:** the per-rect
texture arrives with authored (DSL) cold lights, alongside a true baked-cold tier ([D-5](#d-5)).
