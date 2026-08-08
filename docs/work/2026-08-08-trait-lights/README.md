# Trait lights — things get traits, constant bakes them, emit_light is a trait PARAMETER — 2026-08-08

**What** (user, 2026-08-08, revised at review): things get TRAITS. Trait defs
gain an **`emit_light` parameter** in TOML — no hard-coded list of which traits
emit light and how; any trait may author it. The parameter is **hand-authored
per level**: what color, what intensity, what reach, what fall_off, what
ELEVATION (how far up the light sits — a torch does not emit from its base),
at what level — with room for future variables (direction, flicker…). The
runtime question becomes a pure derivation: *does this object have traits? →
does a trait carry emit_light at its bound level? → then this object emits
that light.* The u16→u8|u8 level split from the first draft is WITHDRAWN —
reach rides the authored table, so the row needs no second byte
([F3](forks.md#f3)).

Traits gain the **constant** feature: a constant trait cannot be assigned at
runtime and cannot be modified at runtime — it is assigned, with its level,
through TOML only. A thing carrying a NON-constant trait cannot be saved to
cold (extra data we do not compress today) — but a constant bind never rides
spacetime at all: static, derivable from the def. That is how lights bake into
cold. The torch gets a constant light-emitting trait.

**Why now**: the `visual.light` block is a one-off presentation lane only the
client reads; a trait parameter is the general mechanism the rest of gameplay
already speaks. Moving light onto traits makes "a glowing anything" one TOML
line, and `constant` gives the corpus a zero-storage lane for static gameplay
data — the first compression law for thing-side traits.

**Design stance**:
- `constant` is an **assignment-site flag** ([F2](forks.md#f2)); keyword
  ratified over `immutable` ([F1](forks.md#f1)).
- `emit_light` is a **def-side per-level parameter table** ([F4](forks.md#f4))
  — the bound level selects the whole authored tuple; the schema accepts
  fall_off now even though the renderer consumes it in a successor
  ([I10](issues.md#i10)).
- Readers see traits through **ONE merged accessor** ([F5](forks.md#f5)):
  constant assignments derive from the def (no storage), runtime rows ride the
  pawn payload exactly as today; the accessor is the enforcement point, and it
  is where "does anything on this object emit light" is answered.
- Things accept **constant assignments only** in v1 ([F6](forks.md#f6));
  runtime thing-traits are the named successor.
- `visual.light` **DELETES** with `thing_light()`'s consumer shape held
  bit-identical ([F7](forks.md#f7)) — cold baking stays free
  ([I3](issues.md#i3)).
- Multiple light-carrying traits ALL attach — pieces past the prim's ≤4 budget
  SPILL into a child prim (prim-as-piece, the graph's own escape hatch;
  billboards keep their authored depth slots, lights spill order-independently
  under max-accumulate; the real bound is bake cost, measured not capped)
  ([F8](forks.md#f8)).

**Exit**: the torch renders identically from its constant trait (screenshot
pair), a TOML-lit pawn kind glows at its authored elevation and the light pans
with the mover, cold boot re-lights the world from corpus + cold rows alone,
the corpus refuses a non-constant thing trait, docs+memory truth pass, bounce;
the user's eyes close the stream.
