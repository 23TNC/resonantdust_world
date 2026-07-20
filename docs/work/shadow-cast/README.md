# Work — shadow-cast (cast shadows into a ping-pong bitfield + incremental updates)

_Opened 2026-07-20. The next de-risking experiment after [`bitfield-rt`](../bitfield-rt/README.md) (which
proved a bitfield survives an RT round-trip AND survives read-modify-write ping-pong). This one casts
**real shadows** into that bitfield and proves **incremental, per-light updates**. Component:
[`client/pixijs`](../../components/client/pixijs/). Builds toward the shadow engine in
[`shadows`](../shadows/README.md); reuses the proven bitfield recipe (unorm RGBA8, A=1, float-mod,
ping-pong — no alpha data)._

## Why — the two things left to prove before the real engine

`bitfield-rt` proved the *storage* (write/read/ping-pong a bitfield). This experiment proves the two
things `shadow-cold` still needs:

1. **Cast shadows into bits** — take real casters (prims) + a light, project their shadows, and record
   "light `k` shadows here" as **bit `k`** of the bitfield.
2. **Incremental updates** — when a light moves, re-cast **only that light** (and only the prims in its
   radius), carrying the other lights' bits forward untouched. This is the whole point of a bitfield:
   updating one light doesn't disturb the others.

If both hold, we can cast many lights' shadows into one RT and update them cheaply — the core of the
tiered engine.

## The experiment

- **5 cold lights** spawned in the zone at focus **(100, 50)**. Each light owns **one bit** — **5 bits in
  ONE channel** (the RED byte, bits 0–4). (One channel is deliberate: proves 5 lights in a single byte;
  RGB → 24 is the same pattern widened, and **A is never used** — [`rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md).)
- **Cast** each light's shadows: for every prim within that light's radius, project a **billboard-quad
  shadow** (simplest — the projected-silhouette refinement is deferred, as in [shadows D-3](../shadows/deviations.md#d-3))
  and rasterise its coverage. Set the light's bit where its shadow falls.
- **Two bitfield RTs, `shadow-a` / `shadow-b`, ping-ponged** (the proven pattern). An update **reads** the
  current one and **writes** the other, then displays the one just written; the next update swaps.
- **Display** the bitfield as **5 distinct colours** (one per light-bit; overlapping shadows add).
- **Move one light every second** (round-robin). That move is the only **dirty** light that frame.

## The update — carry-forward + re-cast only the dirty light (no feedback loop)

Each update (≈1/second, when a light moves) is **two passes into the destination buffer** — never reading
the buffer it writes (the [feedback-loop rule](../shadows/issues.md#i-8) — ping-pong makes source ≠ dest):

1. **Cast pass →`mask`:** rasterise the dirty light `k`'s billboard shadows (its in-radius prims only)
   into a small **coverage `mask` RT** (`max` blend, union of prims). This is a mini `shadow-hot`.
2. **Combine pass → dest:** a full-screen pass reads **`src` (the current bitfield) + `mask`** and writes
   **`dest`**: `dest = (src with bit k cleared) | (mask>0 ? bit k : 0)`; the **other 4 bits are copied
   from `src` unchanged** (carry-forward). Then swap `src`↔`dest` and display `dest`.

So only the moved light re-rasterises; the other four ride through untouched. **Source ≠ destination** in
both passes, so there's no framebuffer feedback. (Between updates, the last-written buffer is just
displayed — no ping-pong needed on idle frames.)

This is a miniature of the real pipeline: `mask` = `shadow-hot` (a light's coverage), `shadow-a`/`-b` =
`shadow-cold` (the ping-ponged bitfield). Proving it here proves that seam.

## What it proves

- **Shadow-casting into a bitfield** — 5 lights' shadows coexist as 5 bits in one channel, each decodable.
- **Incremental updates** — moving one light re-casts only it; the other four bits are provably undisturbed
  (their colours don't flicker when a different light moves).
- **Ping-pong under real updates** — read `src` → write `dest` → display `dest` → swap, driven by moves.

## Latitude

"Pass data however is easiest." Suggested: **screen-space** casting (project the lights + in-radius prims
world→screen each update, rasterise there) — world-space toroidal storage is already proven by
`bitfield-rt` E1–E4 and isn't what this experiment tests. Casters = the scene's standing prims (things) in
radius (re-add the `standingPrims` enumerator the nuke removed), or a few planted test prims if that's
simpler. Shadow shape = a plain billboard quad projected from the light — fidelity isn't the point here.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); gotchas in [`issues.md`](issues.md).
Throwaway by intent — on PASS its findings graduate into [`shadows`](../shadows/README.md) and the folder
can be archived (as `bitfield-rt` was). **No alpha channel is used for anything but colour.**
