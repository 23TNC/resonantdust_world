# Completed — npc-host

## 2026-08-09 — P4 (first half) + P5: the control arc + the truth

**The control arc (I7)** — wolves + bunnies modules, fed by their own stage-1 AI, NO forced
bands, survival's drains live: SIX 5-minute samples over 30 minutes, wolf_count = 2 and
bunny_count = 3 FLAT throughout (the quantized rows 0x2000/0x3000 unmoved), the host up the
whole arc. **P5 the truth** — memory `npc-host-delivered` + the MEMORY.md line written;
docs-check green (the 5 standing warnings pre-date the stream). The bounce criterion is
covered by the stream itself: seven edge deploys, three wiping-adjacent module republishes,
full sim restarts and host restarts — the arcs re-proven after EACH (the host self-healed
through every one; the browser was re-eyed after the wasm sweep heal, capture migrate3).
NOT done, recorded plainly: the hosted wolves have no HUNT (the wolves brain remains the
stage-2 reference); brain emotions/conditions are authored-ready but unread by DECIDE
(stage 3's seam); multi-host arbitration untested (one host ran). The user's eyes close
the stream.

## 2026-08-09 — P4 (second half): the harness re-proof (I5)

`NPC_BRAIN=debug NPC_KIND=debug_mover NPC_COUNT=8` under the single-brain FALLBACK: 8 movers
minted and wandered (175 move intents in ~3 min) and the worker composed IN STEP with the
master (compose tic = master+1 throughout the sample — mover-perf's verdict language). The
harness lane survived the host restructure untouched; the drill movers were culled after.

## 2026-08-09 — P3 (second half): ONE policy, both kinds; the legacy containers retire

The group brain PARAMETERIZED by governed kind — the diet was never code (the corpus's
affordance gates decide what each kind eats), so "wolf" vs "bunny" is the whole difference;
wolf_pack runs it with group_size 2 (its level-1 trait), bunny_fluffle with 3. FOUND + FIXED
live: the SNAPSHOT RACE — a resting pawn's StateObject replays once at subscribe and can
beat the Owned frame, orphaning ownership forever; the world model now keeps a pawn-position
map (`Bot::pawn_at`) and the OwnedPawn arm adopts from it directly (six pawns adopted
"from the world model" at the next boot, both groups at target within seconds). The LEGACY
wolves/bunnies CONTAINERS retired — the host is the wildlife driver; their unowned strays
take I8's stated fate (the world reclaims them). The wolves BRAIN stays in-tree solely as
the HUNT-lane reference (stage-2 world-state AI's seam) and `debug` remains THE measurement
harness; the kind-scan adoption no longer drives anything (P2 item 2's tail). wolf_pack's
count row 0x2000 (=2), fluffle's 0x3000 (=3) — both exactly their traits' numbers.

## 2026-08-09 — P2 (fourth piece): position, area, migration

The F3 clamp: `in_area` (cheb ≤ radius from the module's center) bounds the fluffle's
water/food SCANS (the None fall-through IS the I6 wander degrade); wander/mint targets were
already area-picked. THE MIGRATION DRILL: the host restarted with bunny_fluffle's anchor
moved (116,71)→(124,75) — F2's law verbatim, the named anchor re-sent at new tiles; the
owned bunnies re-attached via the ownership fan and, by successive area-targeted commands
alone, ALL THREE relocated inside the new radius within ~4 minutes (positions probed:
(119,70)/(119,72)/(123,77)) — migration EMERGENT, no herding verb. On camera at the new
center (capture migrate3: B glyphs in the fresh area). Found + fixed on the way: the wasm
loader had missed the brains.toml consumer sweep (boot refused `[[brain]]` — I3's bite,
black screen) — shared/webgl rebuilt with the OwnedPawn arms (web.rs + wasm lib) added.

## 2026-08-09 — P3 (first half): the hosted group brain — the F8 loop LIVE

The ACTOR seam threaded through every brain (`on_event`/`tick` + helpers take
`act: &Client` — sense reads the shared world, acts queue on the MODULE's session, so
attribution flows by construction); `Bunnies::hosted` (explicit area/count, ownership-GATED
adoption fed by `Event::OwnedPawn`, owned pruned on death) + `init_with` (the host's corpus)
+ `set_group` (the F8 bookkeeping target); `run_host` builds the bunny_fluffle group brain
with `group_size` from its brain def's trait, fans world events read-only to every brain,
learns each module's player-pawn off its own fan, and ticks per module. The count need
writes on drift. GUARD FIX (found live): the legacy created-counter re-roll double-mints on
stream-in lag (torch-perf I9's race, worse under the host) — HOSTED mode paces on the
OWNERSHIP LEDGER instead (owned grows only on real mints, prunes on death; can never
double-mint). Verified LIVE, the full loop: count 0 written → bunny_count zeroed (packless
bands server-side) → mints via the module session → Owned fan → gated adoption (legacy
bunnies IGNORED) → count 3 = the trait's group_size (row 0x3000 on 0..16); the DEATH drill:
kill one → "the pack thins" → count 2 → ONE re-mint → count 3; a 2-cull settles back to
exactly 3 with no overshoot. OPEN in P3: the wolves conversion onto the shared policy
(item 2) — the fluffle's keep-alive still rides the bunnies' own think().

## 2026-08-09 — P2 (third piece): the ownership re-attach lane

The edge's `fan_owned_pawns` (per session, post-login): subscribes the pawn spawn ledger
WHERE issuer = the session's player and relays `Owned` frames (replay at login + live on new
mints); client/core carries `Event::OwnedPawn`; the host surfaces each module's set.
Verified LIVE: at restart the wolf_pack module logged `module owns pawn 0x30800004` — the
re-attached set equals the minted set exactly (the item's acceptance). STILL OPEN on the
item: deleting the legacy brains' kind-scan adoption — gated on their conversion to modules
(P3's policy), so the box stays open until then.

## 2026-08-09 — P2 (second half): the ISSUER attribution (I11 built)

The full chain: `queue`/`queue_at` take `issuer_player_id` (the edge stamps its session's
player; worker continuations pass 0), `EventLog` carries it, the worker's CREATE +
SPAWN_REQUEST arms forward the causing event's issuer into `spawn(… issuer)`, and BOTH
spawn ledgers (pawn + player_pawn) record it — ownership is now queryable server-side.
Modules republished (wiping — the linkage was SQL-cleared so logins re-minted),
st-bindings + edge bindings regenerated, every consumer rebuilt, the host self-healed
through the bounce. Verified LIVE: the player-pawn ledger shows every mint's owner; the
pawn ledger shows the legacy brains' mints stamped AUTOMATICALLY (Wolves→1025,
Bunnies→1024) and a drill SPAWN_REQUEST queued as `wolf_pack` (1033) minting pawn
0x30800004 with issuer 1033. REMAINING for item 2's tick: the re-attach lane (fan "your
mints" to the owning session) + deleting the kind-scan adoption.

## 2026-08-09 — P2 (first half): the F10 mint + the host skeleton

**The login-name mint (F10)** — the edge funnel resolves a brain def whose name equals the
player's name FIRST, deriving its granted needs; verified live (a `wolf_pack` login minted
def 0x90010010 + wolf_count, fanned). **The host** — `npc::run_host` (NPC_MODULES=
`"wolf_pack@112,68;bunny_fluffle@116,71"`; single-brain envs untouched — I5): ONE world
session (`npc_host`) carrying every module's NAMED anchor (F2 — the engine merges them into
the one zone set) + one command-only PLAYER session per module (F1, the spike-b posture);
the module's radius reads from its brain's `area_of_influence` bind through
`player_trait_stat`. Verified live (container rd-host): both module-players up with
corpus-derived radii (wolf_pack 12.0 = level 2, bunny_fluffle 8.0 = level 1), each
receiving ITS OWN player-pawn's row — wolf_count (key 0x50) vs bunny_count (key 0x60), the
per-brain derivation proven end to end; players rows server-side. Group behaviors land on
this skeleton with P3's policy (the plan's own structure).

## 2026-08-08 — P1: brains as content

**codec** — `TYPE_BRAIN = 9`. **loader** — `[[brain]]` (BrainToml/BrainDef: needs + CONSTANT
player-trait binds ONLY, type must be `brain`, unknown needs/non-player-traits refused);
`Bundle.brains` + accessors (`brain_needs`, `brain_player_traits`,
`brain_definition_reference` — the login-name lookup, seed-packed) + `player_trait_stat`
(the F10 parameter read from BINDS — player-trait rows can never enter the trait lanes, and
constant binds need no rows). **content** — `brains.toml`: wolf_pack (wolf_count;
group_size 2 @L1, area_radius 12 @L2) + bunny_fluffle (bunny_count; 3 @L1, 8 @L1);
`players.toml`: bunny_count (packless band shared), the group_size/area_radius STATS, the
three player traits with per-level `stats` contributions; the default `player` def carries
NO binds now (the drill test updated). **master** — the brain lane in `allocations` +
`type_id_of`/`subtype_id_of` arms + the `player_trait` category (previously resolving by
fallback only). Verified: 70+2+2 content tests green (goldens re-blessed; the new pin
asserts TYPE_BRAIN packing, granted needs, constant binds, and the per-level params
2/3 group, 12/8 radius); registry seeded 248 defs, ZERO divergence, brain rows at
0x90010010/0x90010020 — exactly the seed derivation.

## 2026-08-08 — P0: the paper + the spikes

**The paper** — VARIABLES.md gained §Brains: `TYPE_BRAIN = 9`, `[[brain]]` defs carrying
needs + constant player-trait binds ONLY (never world objects), the LOGIN-NAME def
resolution law (one resolution point, character-select generalizes it), parameters riding
the STAT lane (per-level `stats` contributions — zero new machinery), and the wild-pawn fate
STATED: the world reclaims the ownerless via survival's drains; a steward brain is the named
successor. docs-check green. **The spikes (F9)** — (a) answered by the delivered
player-pawns stream; (b) proven live: an anchor-less probe session's SPAWN_REQUEST minted a
bunny (worker log tic=43531) — module-players are command-only logins over the host's ONE
world model.
