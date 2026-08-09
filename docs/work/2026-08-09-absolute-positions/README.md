# absolute-positions — THE 48-BIT LAW; one absolute coordinate for everything

> **CLOSED unexecuted (user, 2026-08-09): "we're not re-designing everything."** The design
> is preserved intact — the law, the spike plan, the ladder — with no obligation attached.
> The existing nibble hierarchy is correct and shipped; this folder is a recorded option,
> not a debt. Reopen only if it is ever genuinely wanted.

_User (2026-08-09): "Well that changes everything." The u32 self-limit existed because JS had
no u64 — but everything passes as f64, so the real ceiling is 48 bits (53 with margin). With
the law in place the clever optimizations stop being necessary. Positions become
`u64 = dead:16 | realm:16 | unit:32` (unit = `x:16 | y:16`) — every single thing gets an
ABSOLUTE coordinate, unique within its realm; realms bump u8 → u16. Pack so subscription is
MATHEMATICAL: `min_row < position < max_row AND min_col < position < max_col AND
min_region < position < max_region`. Everything changes — this is going to be so much work._

## What this retires (the survey is the codebase we built)

The entire NIBBLE HIERARCHY is a u32-era compression: `macro_position_reference` (u16 zone
address), `micro_position_reference`, `position_reference` packing, the u4 realm→region→
zone→tile ladder (ZONE_DIM/REGION_DIM/REALM_DIM = 16), zone-keyed subscriptions (the edge's
per-zone streams, the client's anchor→zone-set machinery, ColdTiles' 256-cell rows, the
event fan's `zones`, the orchestrator's zone grouping, worldgen's per-zone seeding, the npc
world model's zone maps). Every one of those exists to fit position into small words. With
48 bits, position is just... position.

## The stance

- **THE 48-BIT LAW is named law** (F1): any single value crossing a JS boundary uses ≤ 48
  of its u64 (dead:16 asserted ZERO; TS splits by division, never `>>> 32`). u32 remains
  the WORD for GPU/TypedArray/action-stream lanes (those layers are truly 32-bit). The
  existing u64 keys already conform (state_uid/spawn_uid/cold_uid = 48 used); `event_uid`
  is the one violator (64 used) and gets brought under the law.
- **The position** (F2): `dead:16 | realm:16 | unit_x:16 | unit_y:16` — the UNIT is the
  SUBTILE SIXTEENTH (chord movement's precision, native): 65536 sixteenths = 4096 tiles per
  realm axis, exactly today's realm span with sub-tile baked in. 65536 realms. One u64
  addresses every position in the world absolutely; the tile is `unit >> 4`.
- **Subscription becomes arithmetic** (F3): interest = range predicates over coordinate
  COLUMNS (`x BETWEEN … AND y BETWEEN …`), not zone enumeration — the spike (P1) proves
  spacetime serves banded 2D ranges efficiently before anything else hardens.
- **Zones survive only where they earn it** (F4): as WORLDGEN cells (generation batches by
  16×16 tiles) and nothing else. As an ADDRESS, as a SUBSCRIPTION key, as a GROUPING unit —
  retired.
- **This stream builds the FOUNDATION + one pilot lane; the world migrates on a LADDER**
  (F5): the law + the paper, the spike, the codec core, and the PAWN HOT LANE end to end on
  absolute positions (mint → move → subscribe → render). Every other subsystem gets a named
  rung in `ladder.md`, executed as successor streams — an epic pretending to be one stream
  would stall; a foundation + a proven pilot + an ordered ladder compounds.

## Exit

The law is written; the range-subscription spike has live numbers; the codec core round-trips
absolute positions; the pawn hot lane runs on them end to end (a pawn mints, walks, streams
into a range subscription, renders) beside the still-zone-keyed rest; the ladder names every
remaining rung with its dependencies. The user's eyes close the stream and pick the first
rung.
