# Completed — npc-host

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
