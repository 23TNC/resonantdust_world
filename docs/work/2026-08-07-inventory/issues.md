# Issues — inventory (anticipated; logged before they bite)

## I1 — the pawn module redeploy WIPES live pawns {#i1}

The `inventory` table + the `remove` extension change the pawn module — the
redeploy clears its transient tables (the food-chain I1 law). Sequence
deliberately: redeploy pawn → restart master → re-mint the cast (/spawn + npc
adoption). The worker's subscription SQL is a string — add
`SELECT * FROM inventory` and verify LIVE (build-gates law).

## I2 — inventories must MINT EMPTY {#i2}

Needs mint at the EFFECTIVE max (food-chain I2) — minting inventory that way
births pawns "full" and `can_carry` refuses forever. The need corpus gains a mint
rule (`mint = "empty"` on the inventory need; default stays full) and
`mint_sidecars` honors it. Unit-test both directions: thirst still full,
inventory zero.

## I3 — count/rows drift {#i3}

The need VALUE caches the row count. One author (the worker's composer), one
atomic program per mutation (INV verb + SET_NEED together — F3) is the defense.
Never author one without the other; if a drill ever shows drift, the bug is a
mutation site skipping the pair, not a sync problem to paper over.

## I4 — new codec verbs reject ASYNC-INVISIBLY until the event shard redeploys {#i4}

The intent-queue-ui law: INV_ADD/INV_REMOVE must ride a redeployed event-shard
module AND the edge verb allowlist (movement-hardening), or queues silently drop
them. One checklist pass: codec → allowlist → module redeploy → shared-crate
hashes → live drill.

## I5 — drop names a slot that can mutate under it {#i5}

A queued drop targets slot N; by fire time the pawn may have picked up / dropped
elsewhere and N holds a different item (or nothing). The inputs carry slot +
EXPECTED item def; the completion re-validates both (the lumberjack re-validation
law) and logs a NO-OP on mismatch. Never fire on "whatever is in N now".

## I6 — the pie menu must GRAY pick_up when full {#i6}

The wasm availability filter evaluates `below_max` — it needs the pawn's need
rows + trait rows for need_bounds at menu-build time. The signature already
carries them (food-chain); verify the browser path actually passes the ACTIVE
pawn's rows, or the menu offers a pick_up the worker then refuses (offered-then-
refused is the exact class input-rework's filter exists to kill).

## I7 — two pawns race one thing {#i7}

Both queue pick_up on the same log; both pass at queue time. The FIRST completion
tombstones the carrier; the second re-validates carrier presence → logged NO-OP,
no phantom item (the lumberjack race law, re-drilled here because store also
writes INV_ADD — the no-op must skip the WHOLE program, not just the SET).

## I8 — registry appends + six consumers {#i8}

pick_up/drop (interactions), the inventory trait + need, can_carry — REGISTRY
appends: master seeds on boot; worker/npc/edge/wasm rebuild; golden re-blessed;
browser wasm cache-busted; /spawn needs a page reload after seeds. The standing
checklist, not vibes.

## I9 — display names {#i9}

The panel prints the TOML `name` verbatim (human_male, plant_matter). Reads fine
for now; a `display = "Human"` corpus field is the recorded successor when it
grates. Not this stream.

## I10 — drop with no empty pathable neighbor must REFUSE {#i10}

Forage's all-full rule is "logged no-yield" — acceptable for generated matter,
ITEM-DESTROYING for drop. Drop refuses instead: the completion no-ops, the item
stays in the slot, the player hears it (log now; UI toast is a successor). The
two spawn callers therefore take a policy flag, not a shared default.

## I11 — the button row is new panel real estate {#i11}

The details panel's top row must not collide with the close affordance / drag
region the panel already has; the inventory panel needs its own z-band slot in
the remain-on-top band (ui-select). Small, but the first place a second button
(character sheet, etc.) will land — name the row a BUTTON ROW, not "the
inventory button", in code.
