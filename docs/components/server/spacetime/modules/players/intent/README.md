# Intent — `players` (what goes in, and why)

_Last updated: 2026-07-15._

## What goes in

`claim_or_login(client_time_ms, name)` — trust-on-first-use: an unknown name creates a player, a
known one just bumps `last_login_secs`. `create_player` registers without logging in.
`set_last_login` is the client's post-subscription stamp.

**This is intentionally insecure.** Anyone can claim any name — no password, no token, no external
check. Replace before exposing it to anyone you don't trust.

Reserved ids (`< FIRST_PLAYER_ID` = 1024) are server-internal and unclaimable by a human, with one
exception: `Developer` (`DEVELOPER_ID` = 512), which `init` seeds with `content-author` pre-granted
so the in-app editor needs no out-of-band grant.

## Who reads it

The **edge**, at login, to resolve a name → `(player_id, player_shard_reference)` and bind the WS
session. It subscribes `SELECT * FROM players` per session so the read-back after the reducer hits
a warm cache.

The module is gate-mediated: reducers take an explicit `player_id` the edge supplies. There is no
Identity-keyed session here — the edge owns the WS → player map.

## Why one row per player

It was a `valid_at` version-history table until 2026-07-15. The history bought nothing: a GC sweep
reaped every prior version every 10 minutes and nothing ever read one. Flattening it also let
`name` become schema-`#[unique]`, which the history schema made impossible (a player's own version
rows collided on it) — so uniqueness lived only in `claim_or_login`'s lookup and any other writer
silently bypassed it.

## Entry / exit

| | |
|---|---|
| in | `claim_or_login`, `create_player`, `set_last_login`; `set_player_faction` / `set_player_permissions` (gate-authorized) |
| out | read by the edge at login |
| seed | `init` provisions `Developer` |
