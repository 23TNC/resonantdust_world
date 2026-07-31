# Forks — lighting + shader rework

_Decision points, options, which we chose and why. F1–F6 are performance adjustments to the user's
design; F7–F10 are error handling. Everything here is an addition to
[`docs/intent/2026-07-31-rework.md`](../../intent/2026-07-31-rework.md), not a change to it — where
this file and that file disagree about a record layout, **that file wins**._

## F1 — Per-light slots for writing, one summed map for reading {#f1}

Eight independently-addressable slots are what make removal exact — clear the slot, no subtraction,
no quantisation. But the display pass only ever wants the **sum**, and reading 8 slots per screen
pixel is ~24 M fetches/frame at 2560×1172 purely to add numbers back together.

- (a) Slots only; the display sums 8 fetches per pixel.
- (b) Summed map only; lose per-light removal (this is the old system).
- **(c) Both: 8 slots (write side) + 1 summed map (read side).**

**Chosen: (c).** Updating the sum is *one blended draw of `new − old`*, and `old` is free because it
is sitting in the slot. That is precisely what made the differential impossible before — nothing
stored the old contribution, so `DIFFERENTIAL_WIRED` stayed `false` and 4 MiB of prev-buffers idled
from creation to deletion. Here the storage exists for an independent reason and the differential
falls out of it.

Cost: one more map. At `RGBA16F` that is 8 MiB against the 64 MiB of slots — cheap for turning the
display's per-pixel cost from 8 fetches into 1, in the pass this stream is most worried about.

## F2 — `RGB10_A2` at ¼ scale, not `RGBA8` {#f2}

A slot must blend (hardware accumulate), stay 4 bytes, and hold a light's contribution.

| | range | blends | bytes |
|---|---|---|---|
| `RGBA8` | 0–1 | core | 4 |
| **`RGB10_A2` at ¼ scale** | **0–4** | core | **4** |
| `RGBA16F` | HDR | needs the float ext | 8 |

**Chosen: `RGB10_A2`, storing `contribution / 4`.** Today's blit clamps at `vec3(4.0)` — **overbright
up to 4× is a shipped behaviour**, not an accident. `RGBA8` would clip each light at 1.0 *before* the
sum, flattening falloff near bright sources; that is a visible change dressed as a storage decision.
Ten bits over a 0–4 range gives 256 levels per unit interval — exactly today's effective precision —
for the same 4 bytes.

It also drops `EXT_float_blend` from the write path. The renderer currently **hard-fails at boot**
without it (`renderer.ts:57`); fixed-point blending is core WebGL2, so the new path has one less way
to not exist on a given machine.

## F3 — One draw for all 8 lights, index derived from fragment x {#f3}

The slot map is 8 px per texel. Writing light `l` has to reach only its own px.

- (a) 8 draws, one per light, offset or `colorMask`ed.
- (b) MRT — one draw, 8 attachments.
- **(c) One draw over the full 8×-wide map; the fragment derives `l = x & 7`.**

**Chosen: (c).** (b) is disqualified outright: **MRT hung Chrome twice** in the previous stream and
was never root-caused — bisected to a single attachment writing a constant and it still hung
([strip I1](../2026-07-31-lighting-strip/issues.md#i1)). Nothing in this stream may use MRT.

(a) costs 8× the draw setup and 8× the vertex work for identical fragment work. (c) is one draw, each
fragment does exactly one light, and it parallelises without any per-light state. The same trick is
why the 8 slots are laid out along x rather than as array layers: contiguous slots are also
cache-friendly for the summing read.

## F4 — The shadow buffer is 3 px per UNIT, and ping-ponged {#f4}

Two things the design leaves implicit, both load-bearing:

**Per unit, not per tile.** The doc's prose says "three px per TILE" but the algorithm reads *adjacent
units* (`±x`, `±y`) and runs 256 fragments per tile. Per-tile would give all 256 fragments one shared
record and destroy the resolution the design is built on. Recorded here because it is a **256×**
memory difference: ~6 MB per buffer at the window size, not 24 KB.

**Ping-pong.** The adjacency step reads neighbouring units' shadow while those fragments are writing
theirs — you cannot read the attachment you are writing. Double-buffer, read last frame, write this
frame.

Staleness is **safe by construction**: adjacency is an accelerator, and a stale miss simply falls
through to the corridor walk, which is authoritative. That is worth stating because it means the
ping-pong costs correctness nothing — but the buffers must be **cleared to 0** on allocation, since
0 = "no caster" = take the slow path, whereas garbage would name a real prim.

## F5 — Gate the per-pixel refine to units that hold a caster {#f5}

The per-pixel pass is the design's one unbounded cost. But most pixels are trivially lit or trivially
shadowed: only a unit whose shadow slot names a caster needs the expensive texture test.

**Chosen: gate on `shadow[...] != 0`,** and **measure the gate's selectivity as a percentage** rather
than assuming it. The old stream planned exactly this gate, never built it, and its cost estimate
(0.05–0.15 ms) was never checked against anything.

## F6 — Reach is derived from intensity, by one shared function {#f6}

`corridor_walk(light)` needs a reach bound and the per-tile `light` set is a *reach* relation — but no
record holds reach.

- (a) Add a reach field (nothing has spare bits; both records are exactly 128).
- **(b) Derive reach from `u10 intensity` via the falloff threshold.**

**Chosen: (b).** Physically, reach *is* the distance at which a light falls below the visible
threshold, so a separate field would be a second source of truth for one fact. `u10` gives 1024
levels, far finer than the old `u12 reach` in units needed to be.

**The binding constraint: it must be ONE function, shared.** The CPU builds the per-tile light set
from it and the GPU bounds the walk with it. If they disagree, a light appears in a tile's set that
the walk will not reach (harmless) or reaches tiles it was never registered in (a shadow that never
gets cast — silent, and exactly the class of bug this project keeps hitting). One implementation, one
place, asserted equal in a dev check.

## F7 — Validate on write, not on read {#f7}

`definition_index + rotation` indexes into a 16-px allocation. A rotation past the allocated count
reads the *next definition's* record and renders a plausible wrong sprite.

- (a) Store a `rotation_count` and clamp on the GPU.
- **(b) Clamp at the CPU writer; assert in dev.**

**Chosen: (b).** There are no spare bits — `definition_data` is exactly 128 across all four channels —
so (a) would force a layout change for a check the GPU should not be paying for per fragment anyway.
The CPU writes `prim_data.rotation` and knows the allocation, so it clamps there; the GPU trusts the
record. General rule for this stream: **the hot loop trusts the data, and the writer earns that
trust.**

## F8 — Sentinel discipline: index 0 is nothing, everywhere {#f8}

The design already reserves index 0. Made explicit as a stream-wide invariant because it is what makes
every other guard cheap: an empty light slot, an unresolved definition, an unoccupied presence slot and
"no caster" are all `0`, so one comparison covers them and no magic value has to be carved out of the
`u16` space.

Consequence to hold: **prim 0 and definition 0 must never be allocated**, and the shadow buffers clear
to 0 ([F4](#f4)).

## F9 — Recycled indices are checked, not trusted {#f9}

A `u16` index into a table with a free list can be recycled: a light slot may name a prim that is no
longer a light, a shadow slot a prim that no longer casts.

**Chosen: verify the type lane at use** — a light checks `emit_type != 0`, a caster checks
`cast_type != 0`. Both lanes live in `BLUE` of a record the walk is already fetching, so the check is
a mask and a compare against a value already in a register.

This is the same reasoning the previous stream reached for the incumbent test ("recycled prim ids are
not a risk, because we re-check the incumbent") — generalised from one call site to a rule.

## F10 — Overflow is counted, never silent {#f10}

Eight lights per tile and eight receivers per tile are hard caps. The eviction rules are nearest-8 by
distance for lights and topmost-8 by layer for receivers.

**Chosen: evict AND count.** A per-frame `droppedLights` / `droppedReceivers` counter, surfaced in the
debug panel.

The old system dropped in silence, and the failure mode is nasty: a light simply is not there, in one
tile, and it reads as a shader bug rather than as a capacity limit. A counter turns a debugging session
into a glance. This is the cheapest error handling in the stream and it is the one most likely to be
skipped, so it is an item with an acceptance criterion rather than a note.

## F11 — Emissive, ambient × AO and decay are NOT in this pass {#f11}

The strip removed three things the rework design does not mention: **emissive** (self-lit pixels — a
wolf's eyes in pitch dark, masked off the zdepth composite's R lane), **ambient × AO** (an
omnidirectional floor attenuated by baked occlusion), and **decay/flicker** (ephemeral particle glow in
its own coarse map).

- (a) Carry all three forward in this pass.
- (b) Carry ambient × AO only — it is nearly free.
- **(c) None of them. They are features, and this pass is a lighting-model rework.**

**Chosen: (c),** on the user's steer: _"they're features that are not implemented in this pass."_
Bundling them would put three independent behaviours inside the one stream whose headline number is
lights-per-frame, and each would blur what the measurement attributes.

**What it costs to add each later**, so the decision is reversible on purpose rather than by luck:

| | to add later |
|---|---|
| **Ambient × AO** | one multiply against `surface.G`, which the blit already samples. Cheapest of the three by a wide margin |
| **Emissive** | a per-prim or per-definition lane — and both records are **exactly 128 bits with no spare**, so it costs a layout change, not a field |
| **Decay / flicker** | a whole extra map and pass; genuinely independent of the lighting model |

**The one thing to not design out:** keep the blit sampling the `surface` composite. If it stops, AO
becomes a re-plumb rather than a multiply, and that is the one of the three most likely to be wanted
back.
