# 2×3 trees + tile occupancy — 2026-08-01

_Components: the content corpus (`content/{data,biome}/`), the worker (`server/worker`),
and [`client/webgl`](../../components/client/webgl/).
The user's directive, verbatim: **"We are going to turn trees into primitives that
occupy 2x3 tiles. This will change their size, and it will require changes to our biome
generation to properly space them. Trees will physically occupy the bottom 2 tiles. We
will develop a method to handle tile occupancy so that we can improve our pathfinding
next turn."**_

**Pathfinding itself is NEXT TURN's stream** — this one delivers the occupancy method it
will consume, verified by probe, consumed by nothing yet.

## The shape

A tree's VISUAL is 2 tiles wide × 3 tall; its PHYSICAL footprint is the bottom 2 tiles —
the 2×1 base row under the trunk. Canopies may overlap anything; footprints are the
thing (and later the pathfinder's obstacle).

- **Masters stay SQUARE pow2** (the settled texture model — aspect plumbing was removed
  deliberately): the conifer authors `size 3` (a 3×3-tile drawn box) with the SUBJECT
  letterboxed to 2×3 inside it. The opaque bbox already measures the true 2-wide
  silhouette, so the shadow card, receiver coverage and presence registration all come
  out 2×3 with no format change.
- **Footprint is DATA, not presentation**: authored in `content/data/things.rd`
  (`&thing.footprint.w 2`, `&thing.footprint.h 1`, anchored at the sprite's base row),
  loaded by the shared DSL, consumed by the worker. The visual corpus keeps only `size`.

## Spacing — a pure per-tile tournament

Worldgen is per-tile and pure (`^biome`/`^rand` of global coordinates) — spacing must
not break that. A tile places a tree iff (a) its roll passes the biome's threshold AND
(b) it WINS against every conflicting candidate in its footprint-conflict window: each
neighbour's roll is recomputable from ITS coordinates, so the tournament is a local,
deterministic, cross-zone-correct function — no generation order, no stored state. The
tiebreak is the roll value itself (equal rolls: lower coordinate wins). Placement
probability retunes alongside (a 2-wide tree at the old 0.22 would read as a hedge).

## Occupancy — DERIVED, never authoritative

The zone's things (+ each kind's authored footprint) are the root of truth; occupancy is
a per-zone bitset CACHE the worker derives on zone load and maintains on every mutation
that touches things (worldgen create, SET/BUILD, future removals). It answers one
question — `occupied(zone, tile)` — and is recomputable from the shard at any moment,
so it can never drift into a second source of truth. Pawns do NOT consult it this
stream (movement unchanged); the pathfinding stream reads it next.

## Out of scope

Pathfinding (declared successor); n/s footprint swap for movers (deferred with the
spatial model); non-square masters; retro-fitting occupancy to walls/build (walls are
tiles, not things — their occupancy joins when the pathfinder defines terrain cost).
