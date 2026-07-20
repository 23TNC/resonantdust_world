# Deviations — shadows

_Where this foundation departs from the durable design
([`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md),
[`design/shadows.md`](../../components/client/pixijs/design/shadows.md)). Pre-logged here because they
are **deliberate foundation-scoping**, not accidents — each shrinks a *count*, never the *mechanism*,
and each names the phase that lifts it. A deviation with a strong reason + a documented un-shrink is the
opposite of drift. Format: what the design says → what this stream does → why → un-shrink._

---

## D-1 · `shadow-cold` is a RED byte (6 bits), not a 32-bit bitfield — 2026-07-19

**Design** ([tiered-lighting §Cold](../../components/client/pixijs/intent/tiered-lighting.md)):
`shadow-cold` is a **32-bit** occlusion bitfield across the full RGBA texel (32 cold casters/rect).

**This stream:** stores the bitfield in the **RED byte only**, using **6 of 8 bits**, A held at 1.

**Why:** 6 lights fit one byte; a single data channel keeps the pack/decode shaders minimal while still
exercising bit-set (pack) and bit-test (overlay). **Un-shrink:** pack across G/B/A too (32 bits) — the
combine shader gains a byte-select, the overlay gains 32 bit-tests. (Note: at 32 bits A becomes a data
lane, so that step must switch to the **verbatim non-premultiply blit** the nuked build called its
"bitfield linchpin"; at 6 bits A=1 avoids it.)

## D-2 · `shadow-hot` is RGB (3 lanes), not 2×4 `uChannel` scatter maps — 2026-07-19

**Design** ([tiered-lighting §engine](../../components/client/pixijs/intent/tiered-lighting.md)): the
scatter engine writes **8 lanes** (2 RGBA maps × 4) via the `outColor = uChannel` trick that dodges the
tint premultiply coupling RGB↔A.

**This stream:** one RT, **RGB = 3 lanes**, A free ([F2](forks.md#f2)).

**Why:** 3 plain colour lanes need no `uChannel`/premultiply subtlety and pair 1:1 with the byte's
batches of 3. **Un-shrink:** reclaim the 4th lane (`uChannel`) and add the 2nd map for 8 lanes/batch.

## D-3 · Casters are solid billboard quads, not textured silhouettes — 2026-07-19

**Design** ([design/shadows.md](../../components/client/pixijs/design/shadows.md)): a caster projects a
**5-triangle fan** of its billboard silhouette with per-corner depth, sampling the sprite **alpha as the
shadow mask** via per-triangle UVs (and the `outline` earcut for the dynamic tier).

**This stream:** projects the **4 billboard corners** and draws a **solid 2-triangle quad** — no fan, no
per-corner depth, no UV, no alpha, no `outline`.

**Why:** the RT/stage/pack/decode pipeline is what's being proven; the silhouette is a fragment-level
refinement that layers on without touching that pipeline. A blocky rectangular shadow is the correct
foundation output. **Un-shrink:** swap the solid quad for the 5-tri fan + presence depths + UV-alpha
sampling from `design/shadows.md` (and reconcile its `nsProject` axis note).

## D-4 · Cold lights are 6 debug uniforms, not a per-rect light-data texture — 2026-07-19

**Design** ([tiered-lighting §Cold](../../components/client/pixijs/intent/tiered-lighting.md)): each rect
reads its nearest 32 cold lights from a **per-rect light-data texture** (2 texels/light).

**This stream:** **6 debug lights** in a plain array, passed as uniforms ([F6](forks.md#f6)).

**Why:** there is no content source of cold lights yet, and 6 is all this slice needs. **Un-shrink:** the
per-rect texture arrives with authored (DSL) cold lights — a separate concern from the shadow mechanism.
