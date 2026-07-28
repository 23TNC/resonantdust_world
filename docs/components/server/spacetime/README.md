# Component — `server/spacetime` (the SpacetimeDB workspace)

_Path: `server/spacetime`. **Not deployed** — it holds the modules, the compose stack, and the
bindings generator. Last updated: 2026-07-28._

Each module below is its own deployable component, publishing to
`resonantdust-<env>-<module>-0`. Build/deploy is `rd build spacetime [module]` / `rd deploy module
<name>`.

| module | | |
|---|---|---|
| [`players`](modules/players/) | auth, login, player→shard routing | live |
| [`index`](modules/index/) | directory + presence | live |
| [`chat`](modules/chat/) | the message feed | live (client link missing) |
| [`event_shard`](modules/event_shard/) | the event queue + settled `event`s (`queue`/`queue_at`) | live |
| [`data_shard`](modules/data_shard/) | hot composition slots — the catch-all (first-pawns F1) | live |
| [`pawn`](modules/pawn/) | hot movers (`TYPE_PAWN`) + the `CREATE` spawn machinery | live |
| `tile` | cold ground — dense biome-rows + overlay (no doc folder yet) | live |
| `thing` | cold scatter — sparse biome-rows + overlay (no doc folder yet) | live |

**No module has a `design/` folder.** Table shapes are cross-component and live in
[`TABLES.md`](../../../TABLES.md); bit layouts in [`VARIABLES.md`](../../../VARIABLES.md); the
reasoning in [`notes/`](../../../notes/tables.md). A module doc that restates a column is drift
waiting to happen — these folders hold *why it exists and how it's used*, not what shape it is.

The two shards are one design, split across two components:
[`intent/spacetime-again/`](../../../intent/spacetime-again/README.md) is the flow that binds them.
