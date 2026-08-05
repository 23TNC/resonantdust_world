# Definition reclaim — designed, deliberately not built

_Intent (`server/spacetime/modules/index`). PROPOSED — the design exists so the decision to build it
is made on measured pressure rather than on principle. Work:
[`2026-08-04-definition-registry`](../../../../../../work/2026-08-04-definition-registry/README.md)
[F7](../../../../../../work/2026-08-04-definition-registry/forks.md#f7)._

## What it would do

Every version bump mints a new `definition_reference` and leaves the old row in `index.definitions`
forever, because entities in the world still hold the old id and still behave as it describes
([F6](../../../../../../work/2026-08-04-definition-registry/forks.md#f6)). Versioning therefore
*consumes* `kind_id` space — u12, so 4096 coordinates shared by every version of every kind.

Reclaim is the sweep that gives one back: once nothing anywhere references an id, retire its row and
let a future allocation reuse the coordinate. The user's sketch, verbatim in substance:

> "A simple method might be to check all shards for an obsolete id periodically and gc it if it's
> completely gone from the server. We can reclaim… for example `0x01020304`, and re-allocate it
> later."

## Why it is not built

**Reclaim is the only irreversible step in the whole design.** Everything else is additive: rows are
inserted, never rewritten; a bad allocation is refused at the reducer; a wrong version is a new row.
Reuse is the one operation that can make an existing, correct stored id start meaning something
else — and it does so silently, because nothing checks.

The runway is enormous. 4096 coordinates, consumed at one per kind per **simulation-visible** change
([F12](../../../../../../work/2026-08-04-definition-registry/forks.md#f12) — art edits mint nothing).
At the current ~17 kinds, that is thousands of content revisions before pressure is real.

## Where an id can survive — the part that makes "gone" hard

"Gone from all shards" is not "gone". Every one of these must be clear before a coordinate is safe,
and they fail differently:

| Holder | Why it is easy to miss | Failure if missed |
|---|---|---|
| `tile` / `thing` dense + overlay rows | the obvious one; a zone nobody has loaded still holds its `kind_reference` | a tile renders as an unrelated kind |
| `pawn` table + payload `PART` entries | pawns store the WHOLE def, twice ([I7](../../../../../../work/2026-08-04-definition-registry/issues.md#i7)) | a pawn adopts the wrong species/kind |
| **`entity_state_log`** | **append-only HISTORY** — it is not "current state" and never gets cleaned | **replay LIES**: an event written when the id meant a conifer replays as moss. The world's past changes retroactively |
| offline / unreachable shards | a shard down for maintenance answers no query, and absence of evidence reads as evidence of absence | the id is reclaimed while a whole region still uses it |
| backups and snapshots | outside the live system entirely | a restore resurrects entities pointing at a reused id |
| stale client registries | a client holds its table until the next fetch ([F9](../../../../../../work/2026-08-04-definition-registry/forks.md#f9)) | a client places an object using a coordinate that has changed meaning |

The event log is the one that turns a leak into corruption. A leaked id costs a number; a reclaimed
id that history still references costs the truth of the history.

## What building it would require

1. **A definition of "referenced" that includes history**, or an explicit decision that history is
   allowed to lie past some horizon (the event log already has retention GC — reclaim could be
   gated on an id being older than that window *and* absent from live state).
2. **A liveness protocol across shards** — a shard that cannot answer must block the sweep, not be
   skipped. Absence of evidence is the failure mode here.
3. **A quarantine tier.** Never reclaim straight from "unreferenced" to "reusable": mark the row
   retired, keep it unallocatable for a long interval, and only then release. The interval is what
   buys back a mistake.
4. **A counter first** — how much `kind_id` space versioning has actually consumed. Build reclaim
   when that number says to, not before.

## The cheaper alternative, if pressure ever arrives

Widening `kind_id` is not available — the packed layout is FROZEN
([F13](../../../../../../work/2026-08-04-definition-registry/forks.md#f13)) and the u12 is spoken for.
But **retiring a kind's whole lineage** is: if `conifer` is deleted from the corpus outright, its
v0..vN coordinates are dead together, and reclaiming a contiguous lineage of a deleted kind is a far
smaller claim than reclaiming one version of a live one. That is the shape to build first if it
comes to it.
