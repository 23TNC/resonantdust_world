# trait-rows-u32 — full-reference rows; trait categories; ACTIVE traits + sprint

_User (2026-08-09): instead of holding our traits as u16 in various places, use u32 — so the
full u12 SUBTYPE lane distinguishes trait categories. Make `[player, pawn]-trait-[constant,
active, passive]`; move constant traits into constant, passive into passive. Pawns get 3
ACTIVE trait slots (players 3 for now — uncertain). Conditions become u32 too, for the same
reason. Add `sprint` as an active trait: activation applies a `sprint` condition that raises
the pawn's speed while the buff runs, and applies a cooldown condition that blocks
re-activation until it expires — the cooldown also carries the exhaustion from the sprint._

## What this dissolves (surveyed against the delivered state)

The packed gameplay row spends its u32 as `data:16 | kind:12 | variant:4` — the identity half
is a bare u16 that CANNOT name its category (stat-model F1). That limitation forced two
v1 postures this week: player traits are CONSTANT-only in a dedicated lane (player-pawns F4 —
"no stored row can name the category until a payload opcode does"), and their stat params are
read from BINDS, never rows. Holding the identity as the FULL u32 `definition_reference`
(type:4 | subtype:12 | kind:12 | variant:4) makes every stored row self-describing — the
subtype lane carries the category, the dedicated-lane workarounds retire, and new trait
categories are pure content.

## The stance

- **LEVEL IS REMOVED; the variant nibble is the tier** (F1/F6, after the level census —
  deepest table 3, highest bind 2, u4 gives 15): walks tier 2 = the walks def at variant 2;
  markers at variant 0; per-level arrays become per-variant authoring sugar; binds rename
  `level` → `variant`; "level 0 = absent" dies (absence is the absent row).
- **EVERY row is the ONE u64 shape: `dead:16 | data:16 | reference:32`** (F1/F7): full
  subtype + variant lanes on all three families; 48 significant bits = ONE lossless f64
  through wasm and JSON (I9: dead asserted ZERO; TS splits by division, never `>>> 32`).
  The data u16 is def-interpreted (F7): need = value, condition = remaining (or declared
  lanes like 2 × i8); TRAIT = zero, reserved for per-instance state (active-trait charges
  are the plausible first customer). The FULL consumer sweep follows (the ONE eval, worker,
  wasm, webgl, npc, panels).
- **Six categories replace two** (F2): `pawn_trait_constant` / `pawn_trait_active` /
  `pawn_trait_passive` / `player_trait_constant` / `player_trait_active` /
  `player_trait_passive`, appended to the gameplay palette; today's `trait` and
  `player_trait` categories RETIRE (their registry rows remain — append-only — but the corpus
  re-authors every def under its new category and no row references the old ones after the
  content migration).
- **Active traits are ACTIVATABLE abilities** (F3): a carrier may bind at most 3 active
  traits (pawns 3; players 3 for now — revisit); activation is a client intent the WORKER
  validates (slot count, the availability predicate) and executes by GRANTING the trait's
  authored conditions. No new condition machinery: sprint's whole behavior is two
  simultaneous grants with different durations.
- **Sprint** (F4): active trait → on activation grants `sprinting` (short, its `stats`
  modifier RAISES ground_speed through the ONE combiner) AND `sprint_cooldown` (long —
  covering sprint + recovery — whose presence gates re-activation and whose own modifiers
  carry the exhaustion: a stamina-flavored need/stat penalty). Re-activation refusal is a
  PREDICATE over the carrier's conditions, not a new mechanism.

## Exit

Every stored row is the ONE u64 shape (tier in the variant nibble, mixed categories
coexisting without collision, data beside full references); the
corpus authors under the six categories with the old two empty; a pawn activates sprint on
camera — visibly faster while `sprinting` runs, refused while `sprint_cooldown` runs, the
exhaustion visible on its cards; the 3-slot law refuses a 4th active bind at load and a 4th
activation at run. The user's eyes close the stream.
