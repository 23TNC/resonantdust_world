# Render performance — stop blocking on art — 2026-08-02

_Component: [`client/webgl`](../../components/client/webgl/). Plan in [`todo.md`](todo.md); decisions
in [`forks.md`](forks.md); what is suspect in [`issues.md`](issues.md)._

## The call

> "There are lots of performance concerns here … we will chase performance improvements. The first
> thing we are going to do is remove the await for all of the textures to load, there is a reason that
> was async and making it sync breaks us. We will load using flat geometry until we get the real
> texture." — user, 2026-08-02

## Why the first item is the await

`TextureResolver.ensureCoPack` gates a stem's entire reveal on **all four maps**:

```ts
const bytes = await Promise.all(order.map((m, i) => (want[i] ? this.loadMapBytes(...) : ...)));
const bmps  = await Promise.all(bytes.map((b) => (b ? createImageBitmap(...) : ...)));
```

Four network fetches **and** four `createImageBitmap` decodes must all finish before a single pixel
of that stem draws. Nothing partial, nothing progressive, and one slow map holds the other three
hostage — `layers` is the least visually important of the four and can stall the stem that needs
`albedo`.

**The asynchrony was load-bearing and was flattened into a barrier.** The renderer already has the
right answer for "art isn't here yet": the GEO tier — flat tinted geometry, no network, no atlas.
`Viewport.channels` falls back to it unconditionally:

```ts
if (alb.geo || surf.geo || !alb.frame || !surf.frame) return solid(alb.frame ?? wf);
```

So the world can draw immediately and upgrade per stem as art lands. The barrier is what turns that
into a wait.

## Scope

This stream is **performance only**. It does not change what is drawn once art has landed, and it
must not touch the subframe geometry that
[`2026-08-02-subframe-ingest`](../2026-08-02-subframe-ingest/README.md) is mid-way through — that
stream owns placement, this one owns *when* pixels arrive and what they cost.

## What the instrument said, and why that is not the whole story

A zoom sweep on 2026-08-02 measured steady-state frames at **3.2 ms**, partition transitions at
**0.2 ms** with zero re-bakes, flat from zoom 0.5 to 4. I read that as "no performance problem" and
was wrong to generalise from it:

- it drove `__zoom()`, **not the mouse wheel**, which takes a different path through `camera.setZoom`
  + `bridge.zoomTo`;
- it ran in a near-empty scene — one torch, a handful of movers, one zone resident;
- it measured **frame time**, which is silent about load latency, decode stalls, GC pressure and
  memory ceilings — and load latency is the symptom the user actually reported.

So `__framecost()` stays the instrument for *frame* work, and this stream needs its own numbers for
everything else ([P0](todo.md)). A measurement that answers a different question than the one asked
is worse than no measurement, because it sounds like an answer.

## The numbers this stream moves

1. **Time to first drawn pixel** after a cold reload, and time to first *real* art per stem. Today
   the first is gated on the second.
2. **Frame cost under load** — a populated view, wheel-driven zoom, not a quiet one.
3. **Resident bytes.** The composites alone are 8 maps × 38.25 MiB ≈ **306 MiB** (4352 × 2304 × 4,
   fixed at every zoom), before atlas pages. That is the ceiling that decides what hardware runs this.
