# Current — `chat`

_Last updated: 2026-07-15._

**The table and reducer are live. The client link is not.** `client/core`'s chat subscription is a
stub that never fires, so the general feed is always empty
(`client/pixijs/src/scenes/world/WorldScene.ts`). Locally-parsed slash commands still work; an
unknown one echoes a system line.

Re-keyed 2026-07-15: `sent_at` (a packed `valid_at`) → `message_id` (`auto_inc`, monotonic =
chronological) + `sent_at_ms` (a plain indexed timestamp for the retention filter). `sequence.rs`,
which existed only to fill `valid_at`'s low 16 bits, is gone.

`sender_player_id` is still unvalidated — see [`intent/`](../intent/).
