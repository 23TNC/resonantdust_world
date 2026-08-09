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

## I9 — the 48-bit transport law (dead 16 = ZERO; no `>>> 32` in TS)

`dead:16 | data:16 | reference:32` = 48 significant bits — INSIDE f64's 2^53, so a row rides
the wasm boundary and JSON frames as ONE lossless number, exactly as u32 rows always did.
Two laws keep it true: (a) the dead 16 is ASSERTED ZERO at every boundary (waking it is a
deliberate future revisit that must re-decide transport); (b) TS splits the halves by
DIVISION (`Math.floor(row / 2**32)` for the reference, `row % 2**16` &c for data) — JS
bitwise ops truncate at 32 bits, so a `row >>> 32` is silently wrong. Grep the sweep for
bitwise on row values before calling it done.

## I10 — the encoding declarations need a refusal posture

A def-interpreted data lane (F7) means a consumer can meet a row whose def declares an
encoding it does not know (version skew across the content cliff). The decoder REFUSES
loudly (log + skip the row) rather than misreading lanes — the geo-flash/unknown-open
precedents: degrade visibly, never reinterpret.
