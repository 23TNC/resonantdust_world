# Issues — trait-rows-u32 (anticipated inventory)

## I1 — this is the widest consumer sweep yet; let the compiler find it

The row helpers' signature change breaks every consumer at compile — worker, npc, wasm,
webgl, the panels, the goldens, mint_sidecars, the crossing scheduler, the sprint-new
activation arm. Budget the full ritual: codec → shared/content → module schema republishes
(needs column u64 — wiping) → st-bindings + edge bindings regen → every binary + wasm/webgl
→ restarts. `deny_unknown_fields` also makes the six new content tables a loader-version
cliff (the survival/brains lesson): old clients black-screen until rebuilt.

## I2 — the world resets; the seed guard and the host re-mint everything

Widening the `needs` column and re-authoring every trait category means the pawn +
player_pawn modules republish WIPING and every minted row re-derives. The host re-mints its
groups (proven self-healing); the login funnel re-mints player-pawns; nothing durable is
lost BY DESIGN — but say it in the plan, not in a surprise.

## I3 — registry hygiene through the category move

Every re-authored def gets a NEW registry row under its new category (append-only). The
retired categories' ids must never be reused, and the content move must land at the END of
each new category's declaration order in ONE commit — a partial move re-seeds mid-order (the
players.toml mid-corpus lesson: 191 divergences).

## I4 — the goldens and the pinned tests all re-bless

The golden fixture, the drill pins (wolf ground_speed 12, the packed-row round-trip test,
player_pawn_drill), and the def-fixture pins in npc all encode the OLD row shape or the old
categories. Each re-blesses DELIBERATELY with the change named — a silent re-bless hides a
real regression.

## I5 — active-slot enforcement has two doors

LOAD refuses a def binding > 3 active traits; the WORKER refuses a 4th ACTIVATE at run.
Both, not either: load can't see runtime grants, run shouldn't be the first to notice an
authored overflow. Players' slot count (3, uncertain) reads from ONE constant so the
revisit is a one-line change.

## I6 — sprint's speedup must actually reach the movement chain

Pacing derives `ground_speed` per hop from the carrier's rows. The sprint condition's stat
modifier flows through the ONE combiner — verify the WORKER's hop derivation includes
CONDITION rows (it reads trait rows today; if conditions don't reach that call site, sprint
would only speed the panel's number). The drill's acceptance is the on-camera speed change,
which catches this lie.

## I7 — the cooldown predicate races its own expiry

An ACTIVATE queued in the same tics the cooldown expires: the worker validates at
PROCESSING (the lumberjack re-validate law), so the refusal is authoritative — but the
client's hotbar should read the same derived remaining to gray the button, or the user
mashes into refusals. The UI half is a SUCCESSOR (hotbar/ability UI); this stream proves the
server law and surfaces refusals in the log.

## I8 — active traits on PLAYERS have no activator surface yet

A player-pawn can bind active traits and the worker will validate ACTIVATE against it, but
nothing user-facing triggers one (no hotbar). The stream proves the lane with a WS-queued
ACTIVATE on a player-pawn; the human-facing surface is a named successor.

## I9 — u64 rows on the wire and in JS

The webgl/wasm boundary passes rows as f64-safe numbers today (u32 fits). A u64 row does NOT
fit f64 beyond 2^53 — but `reserved:16 | data:16 | reference:32` = 48 significant bits, which
fits exactly. Keep the reserved lane ZERO and assert it at the boundary; the day it's used,
the JS surface must move to BigInt or split words.
