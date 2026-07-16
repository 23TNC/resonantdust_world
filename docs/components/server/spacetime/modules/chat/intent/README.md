# Intent — `chat` (what goes in, and why)

_Last updated: 2026-07-15._

## What goes in

`send_chat_message(sender_player_id, sender_name, body)`. Both body and name are validated
server-side; the name is **frozen onto the row** at send time, so a later rename doesn't rewrite
history — that's deliberate, not an oversight.

`general` is the only channel. There is no channel column yet.

## The trust boundary — there isn't one

This module owns no gameplay state and has **no `players` table to validate against**, so it trusts
`sender_player_id` entirely. It is spoofable. The fix is a sidecar, or a chat-side mirror of the
session table — neither exists.

That isolation is the point: chat knows nothing about players, cards, or zones, so it can't be a
dependency of anything. The cost is that it also can't authenticate.

## Who reads it

The **client**, via the wasm core, subscribing `SELECT * FROM chat_messages`. Clients use their
previous `last_login_secs` (from `players`) to pick a scrollback threshold, capped against
`RETENTION_MS` (1h) — whichever is tighter.

## Retention

`chat_retention_sweep` deletes rows older than `RETENTION_MS` every minute. O(N) over the table,
which is fine because retention bounds N: the table stabilises near `send_rate × RETENTION_MS`.
