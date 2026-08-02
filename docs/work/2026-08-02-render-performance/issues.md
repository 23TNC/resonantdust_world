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

## I6 — What SHAPE does the drawn placeholder use? MOOT — [F6](forks.md#f6) abandons drawing {#i6}

[F4](forks.md#f4) draws the placeholder instead of fetching one, which needs a shape before any
texture exists. Two candidates, and the choice changes every item under P3:

- **The authored subframe rect.** Available now, exact, needs nothing new — it is the art's true
  proportions. Yields an outlined tinted box, not a silhouette.
- **A coarse per-kind silhouette, authored beside the subframe.** Much closer to the real asset, at
  the cost of a new authored thing to keep in sync with the art — the drift risk that
  [subframe-ingest I6](../2026-08-02-subframe-ingest/issues.md#i6) already worries about for the
  subframe itself.

**Answered by not needing an answer.** [F6](forks.md#f6) fetches a real 32 px preview instead of
drawing one, so there is no shape to pin. Kept as a record of why the drawn route was explored and
why it lost: a synthesized placeholder is a second rendering of every asset that must keep matching
the first one as the corpus grows, and it measured LARGER than the real art it approximated.

## I7 — The atlas pool cannot free, so every superseded frame leaks {#i7}

`SpritePool` and `MaxRectsPacker` are **insert-only**: `addCoPacked` allocates, and there is no
free, release or evict anywhere in the path. A frame that is superseded — by a better size, or by a
re-master changing the hash — occupies its slot for the life of the atlas.

**Consequences, in order of how soon they bite:**

1. **The preview tier leaks one frame per stem** ([F6](forks.md#f6)). A 32 px co-pack is 64², a
   master co-pack 256², so the steady-state overhead is ~6% of atlas space per stem. Bounded and
   acceptable — but it is a floor that only ever rises.
2. **Per-arrival packing is unaffordable**, which is why P1 packs once per size — see
   [`deviations.md`](deviations.md).
3. **A re-mastered asset leaks its old frame** for the session. `packedHash` notices the change and
   re-packs; nothing reclaims what it replaced.

**The fix is one of two, and they are not equivalent.** A pool free-list (return the rect to
`MaxRectsPacker`'s free list, merge neighbours) reclaims space but fragments. An in-place quadrant
re-blit (draw a newly arrived map into an already allocated frame) avoids allocation entirely and
would ALSO unlock true per-map progressive packing — the thing P1 had to give up. The second is
strictly more useful and is where I would start.

## I8 — A DERIVED preview is slower than the master it precedes {#i8}

The P3 preview tier is built, correct, and **ineffective**, and the reason is the serving model.

Measured on a cold cache with frames driven manually: `packed: 3`, **`prev: 0`, `master: 3`** — the
master won every race. Not a bug in the kick: a manual `resolve()` queues both
`…/conifer/e@32` and `…/conifer/e@256` immediately, and the manifest showed
`'lods': [32, 256]` for that stem afterwards, proving the edge really did derive and cache the 32 px
version on request.

**The physics.** `server/edge/src/textures.rs` derives a requested size *from the master*: read the
master, decode, resize, re-encode, write the cache, serve. The master is a direct file read. So the
preview's critical path **contains** the master's, plus a decode and a resize. It cannot arrive
first on a cold cache. It only wins once some earlier client has already paid to warm it — which is
never true for the case that matters, the first load after a deploy.

I read `textures.rs`'s *"no offline pyramid"* as "none is needed". It means **none exists**, and
that is the problem, not the solution.

**The fix is the user's original instruction, taken literally** (2026-08-02: *"we need to GENERATE
32px preview assets"*). Either:

1. **`bin/art` emits a 32 px co-pack alongside the master** — previews become direct file reads, and
   the cost is paid once at art time. Adds ~2.2–4.3 KB per stem to the repo
   ([F6](forks.md#f6)'s table).
2. **The edge pre-warms its derived cache** for every manifest stem at startup — no repo growth, but
   the first load after every deploy pays for it, and a cold container serves slowly until it
   finishes.

(1) is the honest one: it makes the preview a real asset with a real cost, rather than a latency
that moves around depending on who warmed what. It is also what "generate" meant.

**What still lands from P1/P3 regardless**: the parallel kick, the never-downgrade guard, the
master-only bbox, and [I2](#i2)'s emit-on-failure are all correct and needed the moment previews
become fast. The tier is wired; it is waiting on an asset that does not exist yet.

## I9 — The preview tier cannot be validated on localhost {#i9}

After [I8](#i8)'s pre-warm landed — verified running, `stems=228 derived=1134 failed=0 px=32` — a
cold client still measures **`prev: 0`, `master: 3`**. The preview still never wins.

**This is expected, and it is not a defect in the tier.** Both sizes are now direct file reads, so
the race is decided by request overhead, which is identical for both. What differs is BYTES — 2.2 KB
against ~200 KB — and on localhost bandwidth is effectively infinite, so bytes cost nothing. A
preview tier is a **bandwidth** optimisation; the loopback interface removes the very quantity it
optimises.

So the honest status is: **built, correct, pre-warmed, and unvalidated.** Every mechanism has been
verified in isolation —

- the parallel kick queues `@32` and `@256` in one `resolve()`;
- the edge derives on request (`'lods': [32, 256]` appeared for the requested stem alone);
- the pre-warm derives all 1134 (stem, map) previews at startup;
- the never-downgrade guard is what keeps a late preview off an early master;

— but the end-to-end benefit has not been demonstrated, because the fixture cannot demonstrate it.

**To validate**, throttle the connection (DevTools "Slow 3G", or CDP
`Network.emulateNetworkConditions`) and re-measure `firstPreviewMs` against `firstMasterMs`. On a
link where 200 KB costs real time, the preview should win by roughly the byte ratio. If it does not
win THERE, the tier is genuinely wrong and should be removed rather than kept on faith.

**Do not tune this on localhost.** Making the numbers look better on a fixture that cannot express
the effect is how a placebo ships.
