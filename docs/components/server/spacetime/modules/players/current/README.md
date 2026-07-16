# Current — `players`

_Last updated: 2026-07-15._

**Live and building.** Login works end-to-end (pixijs → gateway → edge → players).

Flat since 2026-07-15: one row per player, PK `player_id`, `name` schema-unique. `valid_at`, the
version history, `gc.rs` (which pruned it) and `sequence.rs` (which fed its key) are all gone;
`init` moved to `lib.rs`.

- `player_profiles.data_shard` is the **last unconverted `data_shard`** — a different concept (the
  auth DB's own partition), left as-is when `Player.data_shard` became `player_shard_reference`.
- `player_profiles` is public and has **no consumer**.
- Republishing needs `--delete-data` — the 2026-07-15 schema change drops existing dev rows.
