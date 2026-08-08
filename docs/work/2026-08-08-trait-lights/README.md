# Trait lights — things get traits, constant bakes them, emit_light replaces the light block — 2026-08-08

**What** (user, 2026-08-08): things get TRAITS. The trait row's u16 `level` splits
into `u8 data | u8 level`. The torch stops being identified as a light-emitting
prim by its `visual.light` block — it gets an `emit_light` trait instead, which
uses **level for color** and **data for reach**, so light can be assigned to ANY
pawn or thing by attaching the trait. Traits gain the **constant** feature:
a constant trait cannot be assigned at runtime and cannot be modified at
runtime — it is assigned, with its data and level, through TOML only. It
follows that a thing carrying a NON-constant trait cannot be saved to cold
(it carries additional data we do not compress at this time) — but a trait
defined in the thing's TOML and marked constant doesn't need to ride spacetime
at all: the data is static, derivable from the def. That is how lights bake
into cold. The torch gets a constant `emit_light` with variables defining
color and reach.

**Why now**: the light block is a one-off presentation lane only the client
reads; a trait is the general mechanism the rest of gameplay already speaks
(affordance tags, stat eval, leveled defs). Moving light onto a trait makes
"a glowing anything" one TOML line, and `constant` gives the corpus a
zero-storage lane for static gameplay data — the first compression law for
thing-side traits.

**Design stance**:
- `constant` is an **assignment-site flag** ([F2](forks.md#f2)) — any trait can
  be bound constant in TOML; the same trait stays grantable (non-constant) at
  runtime on pawns. Keyword ratified over `immutable` ([F1](forks.md#f1)).
- The split is **trait-family only** ([F3](forks.md#f3)): needs keep their u16
  fixed-point value, conditions their u16 remaining-at-write.
- `emit_light`'s level selects an **authored per-level color** on the trait def
  ([F4](forks.md#f4)) — machinery-coherent with the existing per-level trait
  tables and the level-count validation; reach rides `data` in tiles.
- Readers see traits through **ONE merged accessor** ([F5](forks.md#f5)):
  constant assignments derive from the def (no storage), runtime rows ride the
  pawn payload exactly as today; the accessor is the enforcement point.
- Things accept **constant assignments only** in v1 ([F6](forks.md#f6)) — a
  non-constant trait on a thing refuses at load (no thing gameplay storage
  exists; the user's own cold-compression constraint). Runtime thing-traits are
  the named successor.
- `visual.light` **DELETES** ([F7](forks.md#f7)); both torches convert.

**Exit**: the torch renders identically from its constant trait (screenshot
pair), a TOML-constant light glows on a pawn kind and follows the mover, cold
boot re-lights the world from cold rows alone, the corpus refuses a
non-constant thing trait and a runtime write to a constant one, docs+memory
truth pass, bounce; the user's eyes close the stream.
