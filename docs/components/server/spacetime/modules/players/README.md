# Component — `players` (auth)

_Path: `server/spacetime/server/modules/players`. **Live.** Last updated: 2026-07-15._

Accounts, login, and the player→shard routing the client reads at login. Owns no cards or souls —
those live on the shard a player's `player_shard_reference` names.

- **[`intent/`](intent/)** — what goes in, who reads it, the auth posture.
- **[`current/`](current/)** — where it is.

Shape: [`TABLES.md` §`players`](../../../../../TABLES.md).
