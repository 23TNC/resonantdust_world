# Completed — npc-host

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
