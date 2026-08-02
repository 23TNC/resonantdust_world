# Issues — render performance

_What is broken, suspect, or unverified. Numbered so decisions and commits can cite them._

## I1 — Frame time was measured; load latency was reported {#i1}

Recorded because it produced a wrong conclusion on this stream's first day, and the same mistake is
easy to repeat with the same tool.

A zoom sweep measured steady-state frames at **3.2 ms**, partition transitions at **0.2 ms** with
zero dirty slots, flat from zoom 0.5 to 4. I concluded there was no performance problem. That did not
follow:

- it drove `__zoom()`, **not the mouse wheel** — a different path (`camera.setZoom` + `bridge.zoomTo`);
- the scene was near-empty: one torch, a few movers, one zone;
- **frame time is silent about load latency, decode stalls, GC pressure and memory ceilings** — and
  load latency was the reported symptom.

`__framecost()` remains correct for what it measures. The error was generalising from it. Every item
in this stream names the number it moves for exactly this reason.

## I2 — A failed pack never schedules a re-bake {#i2}

`ensureCoPack` ends with:

```ts
if (ok) { this.packedHash.set(stem, hash); this.emit(); }
```

`packCoPack` returns false when the pool cannot allocate — a full 2048² page, an off-page spill. In
that case the bytes are downloaded and decoded, and then **nothing happens**: no listener fires, no
re-bake is scheduled, and the stem stays on the geo tier for the rest of the session with no error
beyond an off-page warning.

That is a plausible cause of "it gets stuck loading" independent of any latency, and it is cheap to
rule out: block the network, watch whether geo paints, then unblock and watch whether stems upgrade.

## I3 — `node_visual` re-runs the DSL hook per lookup, on a per-TILE path {#i3}

Found while widening a struct on 2026-08-02: `node_visual` executes the kind's `@on_create` visual
hook through the DSL VM **on every call**, and `zone_tile_prims` calls it **per tile**. Adding six
formatted key lookups per field to it froze the client outright — 256 tiles × ~9k string allocations.

The freeze was fixed by guarding that one field, but **the underlying shape is unchanged**: a
per-tile call re-executes a VM program to produce a value that depends only on the kind. It should be
resolved once per kind and cached.

This is the clearest evidence that frame-time headroom in a quiet scene says little about behaviour
under load — the cost scales with resident tiles, which is exactly what the 3.2 ms fixture lacked.

## I4 — Nothing tracks resident bytes {#i4}

The composites are 4352 × 2304 × 4 B = **38.25 MiB each**, and there are eight (albedo, normal,
surface, zdepth × cold, warm) = **~306 MiB**, fixed at every zoom level. Atlas pages are 2048² on top
of that, and `lodStats()` — the only reporter of pool occupancy — was deleted with the LOD ladder, so
page count is currently unobservable.

306 MiB is not obviously wrong; it is a consequence of `SQUARE 128`, which was chosen for art
sharpness with the cost measured and accepted (`square-128` P0, ~321 MiB). What is wrong is that
**nothing watches it**, so the next thing that raises it will do so silently.

## I5 — Four maps, one barrier, and `layers` is the least important {#i5}

`ensureCoPack` awaits all four maps before packing any. Beyond the latency, the ordering is
backwards by importance: `albedo` and `surface` are what make a stem drawable, `normal` affects only
lighting, and `layers` only per-material tint. A stem that has albedo and surface could draw and look
nearly right while the other two stream in.

Recorded as the shape of the P1 fix rather than a separate defect: the fix is not merely "do not
await", it is "reveal in importance order".

## I6 — What SHAPE does the drawn placeholder use? UNPINNED {#i6}

[F4](forks.md#f4) draws the placeholder instead of fetching one, which needs a shape before any
texture exists. Two candidates, and the choice changes every item under P3:

- **The authored subframe rect.** Available now, exact, needs nothing new — it is the art's true
  proportions. Yields an outlined tinted box, not a silhouette.
- **A coarse per-kind silhouette, authored beside the subframe.** Much closer to the real asset, at
  the cost of a new authored thing to keep in sync with the art — the drift risk that
  [subframe-ingest I6](../2026-08-02-subframe-ingest/issues.md#i6) already worries about for the
  subframe itself.

**Not blocking**: the rect alone is a large improvement over a flat geo box and can ship first, with
the silhouette as a later refinement. Recorded so the choice is made deliberately rather than by
whichever is convenient when P3 starts.
