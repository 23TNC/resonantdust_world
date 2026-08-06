# Stat model — the gameplay model generalizes

_Opened 2026-08-06 (user): "We have Objects. Some objects are pawns. Pawns have needs, traits,
conditions. Some objects are tiles/things. tiles/things may have interactions. I think
interactions should have affordances, not pawns… we add 'stats' to our list of stuff our pawns
can 'have'. So… stats, needs, traits, conditions. The trait 'Walks' will add N to the pawns
'ground_speed' stat. I believe the affordance is something like 'Can Move Ground', which will
check 'ground_speed > 0'. Then the interaction can have the affordance 'Can Move Ground'. This
way we could 'Can Move Air' or 'Can Teleport' etc." Refined the same day by the planning review —
decisions verbatim in [`forks.md`](forks.md)._

This reworks the model [2026-08-06-interactions](../2026-08-06-interactions/README.md) delivered,
and it comes FIRST: the input stream (active selection, pie menu, interaction-based movement —
recorded, not yet opened) builds on every piece of it.

## The model

A pawn HAS four things ([F1](forks.md#f1), user). Three are stored rows; one is derived:

- **Traits** — leveled stat contributors. Row `kind:12 | variant:4 | level:16` in the payload.
  The toml authors a per-LEVEL table: walks 1 = 60 tics/tile, 2 = 50, 3 = 40 → contributes to
  `ground_speed`. Thing defs bind starting traits (string = level 1, or `{ name, level }`);
  rows mint at CREATE ([F11](forks.md#f11)).
- **Conditions** — timed stat/need modifiers. Row `kind:12 | variant:4 | remaining_at_write:16`
  + the written tic ([F3](forks.md#f3), user — backward-looking, never predicted).
- **Needs** — depleting values. Row `kind:12 | variant:4 | value:16` + `set_tic` in a NEW
  sub-table ([F2](forks.md#f2)) so need updates fan alone; the value is u16 FIXED-POINT over the
  need's authored domain ([F4](forks.md#f4)) and lazy depletion survives intact.
- **Stats** — DERIVED ONLY, never stored, never fanned ([F8](forks.md#f8)): computed anywhere
  from the trait/condition rows + corpus as `clamp(sum of contributions, combined bounds)`.

**The affordance moves onto the INTERACTION** ([F5](forks.md#f5), user) and becomes a stat
PREDICATE: `can_move_ground` ⇔ `ground_speed > 0`, `can_drink` ⇔ `metabolism > 0`. An
interaction lists the affordances that gate it; carriers list the interactions they offer with
their parameters (`interactions = [{ name = "drink", magnitude = 3 }]`, [F9](forks.md#f9));
affordance `variants` and trait-list `requires` DIE. Any consumer holding the corpus + a pawn's
rows can answer "may this pawn do this here" — worker, npc, and the coming pie menu alike.

**One combiner** ([F6](forks.md#f6), user): modifier ranges combine as maximum-of-minimums /
minimum-of-maximums; on an empty intersection the authored winner (`min_wins`/`max_wins`) on the
`[[stat]]`/`[[need]]` def decides, inside authored global safety bounds. Traits/conditions modify
need min/max/RATE through the same machinery, under the re-stamp law ([F7](forks.md#f7)): every
modifier-set mutation re-stamps the affected need rows. The living proof case: `quenched` halves
thirst depletion while active, and the eval integrates piecewise across its expiry
([I4](issues.md#i4)).

## Design stance

- One eval, still — stat derivation, predicate checks, and the reworked `needs_eval` live in
  shared/content and are bit-identical across worker/npc/wasm ([I2](issues.md#i2)).
- Wire stays HUMAN: event inputs remain f32 bit patterns in authored units; the ONE quantization
  to u16 fixed-point happens at write ([F4](forks.md#f4)).
- The corpus stays data: a new stat, trait level table, or predicate is a TOML edit.
- The payload consequence is faced again ([I1](issues.md#i1)): dev pawns re-mint; NEED leaves
  the payload for the sub-table; no old-shape reader survives ([I11](issues.md#i11)).
- Movement is NOT rewired here ([F12](forks.md#f12)): walks/`ground_speed` are authored and
  derived this stream, but the worker's chain still reads `speed` until the input stream replaces
  MOVE_TO with the `move_to` interaction. The handoff is explicit, not drift ([I10](issues.md#i10)).

## Exit

A cold-booted stack re-runs the drink arc on the NEW model, unprompted: trait rows minted at
CREATE, `can_drink` gated by the derived stat, sips quantized to the fixed-point domain,
`quenched`'s rate modifier VISIBLY slowing depletion (piecewise across its expiry), panel cards
green. Captures + logs in `completed.md`; **the user's eyes close the stream**.
