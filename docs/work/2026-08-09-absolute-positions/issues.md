# Issues — absolute-positions (anticipated inventory)

## I1 — the spike is load-bearing; nothing hardens before its numbers

The whole design leans on spacetime serving BANDED 2D RANGE subscriptions efficiently
(F3): live inserts/updates/deletes crossing band edges, tens of concurrent viewports,
mover-rate churn. If range subs underperform equality subs badly, the fallback (row-band
keys) changes the table shapes — so the spike runs BEFORE the codec core is consumed by
anything. The spike's numbers go in completed.md beside the per-zone baseline.

## I2 — subscription updates on MOVING rows

A mover crossing a range boundary must fan a leave (the StateGone lane) exactly as zone
crossings do today. Verify spacetime's subscription diffing handles UPDATE rows entering/
leaving a WHERE band (not just inserts/deletes) — the client's tombstone trust
(movement-hardening) rides on it.

## I3 — the u4 nibble laws are load-bearing in hidden places

`position_macro`/`position_micro`/`pack_position_reference`, autotile math, worldgen's
zone origins, the npc's `macro_world_origin` scans, geo tiers, the anchor radii — the
nibble hierarchy leaks EVERYWHERE. The pilot deliberately does NOT chase them: only the
pawn hot lane converts; every other consumer keeps reading the legacy lanes the worker
still writes (dual-write on the pilot tables is the one transitional cost, retired rung
by rung).

## I4 — chord movement's fractions become native, carefully

Chords interpolate in sixteenths with the clear_point clamp law (I12). The unit-sixteenth
position makes that math native — but the re-anchor cadence, trip serials, and the
resolve-on-touch law all parse today's packed lanes. The pilot converts the WRITE side;
the chord EVAL converts on its own ladder rung with the movement drills re-run.

## I5 — the event fan and orchestrator group by zone today

`EventLog.zones`, the settle fan's per-zone rows, the orchestrator's grouping — all
zone-bucketed. The pilot leaves them (pawn events still resolve zones from positions for
the fan); the events rung re-shapes `event_uid` (the F1 violator) and re-buckets the fan
by RANGE overlap — the deepest rung, dependencies noted in the ladder.

## I6 — worldgen's determinism must not move

Generation seeds by zone cell + per-tile SplitMix64 draws keyed on world position. The
addressing change must reproduce byte-identical worlds for the same seed (the golden
worldgen expectations): F4 keeps generation batching by 16×16 cells, and the rung's
acceptance is an identical-world diff.

## I7 — realm u16: the id exists before any second realm does

The position carries realm:16 but ONLY realm 0 is generated, routed, or served. Nothing in
this stream lights realm 1 — cross-realm routing (index shards, realm_server_reference) is
its own future. Guard: helpers refuse realm != 0 loudly rather than silently computing
into an unserved realm.

## I8 — the JS boundary discipline needs a lint, not vigilance

The law's two rules (dead 16 zero; division not `>>> 32`) will be violated by muscle
memory. Add the grep to docs-check (or a tiny lint) for bitwise ops on known u64-carrying
identifiers in TS — vigilance codified once, not re-remembered per review.

## I9 — the pilot renders beside the zone-keyed world

The webgl client draws movers from zone streams today. The pilot adds a RANGE-fed mover
path while terrain/things stay zone-fed — two subscription models in one client for the
ladder's duration. Keep the pilot path clearly seamed (the range feed owns MOVERS only)
so the mixed period never blurs authority.
