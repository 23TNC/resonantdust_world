# Lighting correctness — pick up the rework and finish it properly — 2026-07-31

_Component: [`client/webgl`](../../components/client/webgl/). Successor to
[`2026-07-31-lighting-rework`](../2026-07-31-lighting-rework/README.md), which built the new
method's machinery and marked itself complete at 39/39 — but its own capability table
([P7](../2026-07-31-lighting-rework/completed.md)) scores **6 of 10**, and the user's verdict is
plainer: it failed to re-implement lighting. This stream closes the gap between "the records
exist" and "the world is lit correctly"._

## What the rework actually left

The good — real, keep it: the flat `u16` prim space, stored caster identity (the thing that
killed the old 9.29 ms re-search), per-light slots + summed map with bit-exact add/remove, no
hot/cold tiers, the corrected measurement harness
([I10](../2026-07-31-lighting-rework/issues.md#i10)), and 16 lights in 2.31 ms of an 8 ms budget.

The gaps — the user's list, mapped to what the code shows:

| user's item | state in the rework |
|---|---|
| reach in prim data | **derived from intensity** (`lightReach.ts`, F6) — no stored field |
| bbox | `definition_data.subframe` written, but scaled/flipped/rotated art was never verified against it (the refine spent its life testing a solid rectangle — [I12](../2026-07-31-lighting-rework/issues.md#i12)) |
| silhouettes | surface-alpha sampling landed LAST COMMIT; unverified against a reference; **receivers are ground-only** and **`cast_type 2` (n/s cards) is defined, not implemented** |
| z-ordering | `presence` is layer-sorted, but nothing proves the lit surface per pixel equals the DRAWN surface — and no billboard receivers means stacking is wrong wherever sprites overlap |
| normal maps | **completely unread** — zero references in `lightPass`/`shadowPass`; ambient×AO also gone |

## Design stances (the user's directives + my resolutions)

**S1 — Reach is STORED, intensity gives up bits.** (User: "I'd like reach in our data, that may
involve reducing the bits allocated to intensity.") `prim_data.B`'s `u10 intensity` splits into
**`u4 reach` (bias +1 → 1..16 tiles) + `u6 intensity` (64 levels over the 0..4 overbright
range)**. Sixteen is not an arbitrary cap: reach is THE measured cost dial (the old fps table:
reach 16 → 18 fps, 8 → 120 with three movers), and 16 tiles is the drill ceiling every headline
number is quoted at — encoding more would be encoding a footgun. `reachFromIntensity` retires;
content re-authors reach explicitly (the old corpus already did — torch 8). Alternative
`u5/u5` recorded in [forks F1](forks.md) if 31-tile reach is ever wanted.

**S2 — The bbox is measured, not trusted.** Each rotation frame's `subframe` comes from the
art's own surface presence at ingest, POST pre-atlas scale; a flipped (west) draw mirrors it;
the writer asserts `subframe ⊆ frame`. P0 first builds the def→(stored, actual) table so fixes
are per-cause, not vibes.

**S3 — Silhouettes: exact, on every axis.** The crossing→caster-frame mapping is proven against
a brute-force per-pixel reference (0 differing pixels, the corridor-identity discipline);
`cast_type 2` implements the n/s perpendicular card through the SAME `base + rotation` def
addressing (the design's whole point — no special case); shadows land ON billboards using the
already-stored `(caster, receiver)` pairs.

**S4 — One z contract.** The prim `layer` lane, the presence sort, the draw's zIndex, and the
refine's receiver pick all derive from ONE ordering, asserted at write. The proof is a
lit-surface probe: colour each pixel by its resolved receiver and diff against the drawn
surface — 0 mismatching pixels on a stacked scene (tile / tree / human body+head).

**S5 — Normals + ambient×AO ride the slot pass.** Per-light N·L samples the RESOLVED receiver's
normal quadrant at slot-write time (the old FINE-lightmap model: the map texel belongs to a
known receiver, so direction × normal bakes into the per-light contribution and the summed map
inherits it — display stays ONE fetch). Ambient×AO returns from `surface.G` in the composite.
Both preserve the differential's bit-exactness (re-verified).

**S6 — The verdict is written, honestly.** Does the new method outperform the old, and why —
measured on the corrected harness with everything above ON, moving lights, reach 16. The
structural argument to verify (already visible in the numbers): (a) stored identity deletes the
old system's dominant cost — re-DISCOVERING the occluder (9.29 of 10.88 ms); (b) per-light
slots make change cost ∝ (changed region × that light) instead of class-wide rebakes; (c) flat
`u16` refs end the 128-bit texel wall that blocked every old extension; (d) fixed-grid passes
scale sub-linearly in N (measured: 16× lights → 2.5× cost). Against it, stated plainly: +12 MiB
resident, and old-vs-new absolute ms are incommensurable (the old numbers were ~4× low — I10),
so the verdict compares structure + like-for-like re-measurements, never the stale absolutes.

## Fixtures

`?user=Claude&focus=104,54&zoom=1` — the placed human `0x30800005` at (104, 54) (the static
fixture; move it via `__bridge.moveEntity`), the npc wolf, the torch pools, the user's walls.
Same harness discipline as the rework: foreground tab, hand-driven ticks, `readPixels` sync
with the MATCHING format, median of runs.

## Out of scope

Emissive, decay/flicker (I6 items 7 + 9) — cut with the strip, restoring them is the user's
call once this stream's shading lands; the open `2026-07-30-pawn-part-placement` stream (its
own plan); any MRT (standing ban); the 8-lights-per-tile cap revisit
([rework I7](../2026-07-31-lighting-rework/issues.md#i7)) unless P6's numbers force it.
