# Forks — absolute-positions

## F1 — THE 48-BIT LAW, and where u32 remains the word

Named law in VARIABLES.md: a SINGLE VALUE crossing any JS boundary (JSON frames, wasm
returns, SQL results) uses at most 48 bits of its u64; the dead 16 are ASSERTED ZERO at
every boundary; TS splits halves by DIVISION (`Math.floor(v / 2**32)`) because JS bitwise
truncates at 32. The law does NOT retire u32 as a WORD: GPU data textures, TypedArrays,
GLSL integer ops, and the action program's `Vec<u32>` vocabulary are genuinely 32-bit
layers — values entering those lanes stay ≤ 32 per word. The audit split is
"transport-bound single" (widen to 48) vs "word-lane citizen" (stays u32). `event_uid`
(macro:16 | tic:16 | ref:32 = 64 used) violates the law today and re-shapes on the event
rung of the ladder.

## F2 — the unit is the SUBTILE SIXTEENTH

`dead:16 | realm:16 | unit_x:16 | unit_y:16`. CONFIRMED BY THE USER at review ("unit means
sixteenth, not tile"): a unit = one SIXTEENTH of a tile —
chord movement's native precision (subtile sixteenths, the I12 clamp law) becomes the
world's atomic coordinate instead of a bolt-on lane. 65536 units = 4096 tiles per realm
axis — exactly today's realm span, so no world re-scale rides along; the tile is
`unit >> 4`, the sixteenth is `unit & 0xF`. 65536 realms (u8 → u16, the user's bump).
Rejected: unit = tile with a separate fraction lane (two lanes to keep in sync — the exact
duality this redesign exists to kill); unit = tile at 65536 tiles/axis (a 16× world
re-scale smuggled into an addressing change).

## F3 — subscription interest is COLUMN RANGES, spiked before anything hardens

The subscription shape: coordinate COLUMNS (`realm`, `x`, `y` — or x/y derived bands) with
btree indexes, interest = `realm = R AND x BETWEEN x0 AND x1 AND y BETWEEN y0 AND y1` —
the user's mathematical form. The P1 SPIKE proves spacetime's subscription engine serves
banded 2D ranges live (insert/update/delete crossing the band edges) and MEASURES it
against today's per-zone equality subs before any table re-keys. Fallback recorded, not
built: row-band keys (a range per y-band over a packed row-major key) if 2D predicates
underperform; Morton/interleave rejected first (range boxes fragment into many intervals —
the opposite of the user's clean min/max form).

## F4 — zones retire as ADDRESS; survive as worldgen batch cells

Generation still batches by 16×16-tile cells (the biome classifier and seeding work in
chunks; that is a THROUGHPUT choice, not an address). Everything else the zone key does —
subscription unit, event fan bucket, orchestrator grouping, cold-row addressing, anchor
sets, npc world maps — retires onto ranges and absolute positions, each on its ladder rung.
The word "zone" leaves the vocabulary at the end of the ladder; until then each rung's docs
say which meaning it retired.

## F5 — foundation + pilot + ladder; not a big bang

This stream lands: the law, the paper, the spike, the codec core (the u64 position type +
range helpers beside the legacy packers), and the PAWN HOT LANE pilot — pawn tables gain
absolute-position columns, the edge serves one range subscription, the client renders
movers from it (mint → move → subscribe → render, the whole vertical). The rest of the
world stays zone-keyed and WORKING beside it. `ladder.md` orders the remaining rungs
(events, cold tiles/things, worldgen handoff, anchors/client, npc, chord-movement
re-anchor, the un-zoning of vocabulary) with dependencies — each a successor stream sized
like the ones this repo actually finishes. Rejected: one mega-stream migrating everything
(nothing ships until everything ships; a stall strands the world half-addressed).

## F6 — dev-posture resets are the migration tool

Every rung may WIPE its shard (republish) and re-derive/re-generate — the sim self-heals,
the host re-mints, worldgen regenerates. No dual-format rows, no in-place data migration,
no version-skew readers: a rung lands whole per subsystem or not at all. This is the same
posture every delivered stream used, stated once for the whole ladder.
