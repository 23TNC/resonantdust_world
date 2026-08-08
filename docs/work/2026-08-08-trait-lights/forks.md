# Forks — trait lights

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — the keyword is `constant`, not `immutable` {#f1}
_2026-08-08 · raised by the user at plan time — theirs to overrule_

**Chosen.** `constant`.

**Why.** Immutable names only half the contract (cannot be modified). The
property doing the real work is stronger: the value is STATICALLY KNOWN FROM
THE DEFINITION — which is exactly what lets the assignment skip spacetime and
bake into cold. That is `const` semantics, not just write-protection: a
runtime-granted trait could be immutable-after-grant and would still need a
stored row. **Rejected — `immutable`**: truthful but weaker; it invites a
future "immutable but runtime-assigned" reading that reopens the storage
question this feature exists to close. (`innate` was the gameplay-flavored
third option — available if the user prefers flavor over precision.)

## F2 — `constant` is an ASSIGNMENT-SITE flag, not a trait-def property {#f2}
_2026-08-08 · resolved at plan time_

**Chosen.** The flag rides the TOML bind:
`traits = [{ name = "emit_light", level = 1, data = 16, constant = true }]`.
The trait DEF stays neutral.

**Why.** The user's own sentence — "ANY trait can be added to things/pawns in
toml and marked constant" — names the assignment as the carrier. It also keeps
`emit_light` grantable non-constant on a pawn at runtime (a spell, a buff)
while the torch's copy is baked. **Rejected — a def-level `constant = true`**:
it would fork every trait into constant/non-constant twins the moment one kind
needs the other flavor.

## F3 — the u8|u8 split is TRAIT-family only {#f3}
_2026-08-08 · resolved at plan time_

**Chosen.** The stored gameplay row stays `data:16 | kind:12 | variant:4`; the
TRAIT family REINTERPRETS its data half as `data:8 | level:8` via new codec
accessors (`pack_trait_data` / `trait_row_level` / `trait_row_data`). Needs
keep the u16 fixed-point value; conditions keep the u16 remaining-at-write.

**Why.** Those two families genuinely spend all 16 bits; traits never have —
every authored level today is 1..15 (the emotions per-level arrays bound them).
**Rejected — splitting the row layout for all families**: breaks needs'
fixed-point lazy math for no gain. **Rejected — widening the row**: the u32 row
is the payload/compression unit everywhere (needs-moodlets F-series).

## F4 — emit_light: `level` SELECTS an authored per-level color; `data` is reach in tiles {#f4}
_2026-08-08 · resolved at plan time — the one real collision found while reading_

**Chosen.** The `emit_light` trait def authors a per-level color table (the
same shape as the emotions' per-level arrays — level 1 = the warm torch color,
level 2 = the blue, append-only); a bind's `level` picks the color, `data`
carries reach in tiles (u8, today's torches use 16). The def also authors the
presentation residue ONCE (intensity, radius, height, flicker, cast/hot flags)
— see [I2](issues.md#i2).

**Why.** Trait binds are validated against the def's AUTHORED LEVEL COUNT
(toml_loader: "trait authors N level(s)") and the emotions machinery already
gives trait defs per-level tables — level-as-color-selector slots straight in.
**Rejected — level as a raw 256-palette index**: the 16×16 OKLCh master palette
is designed but NOT in the repo (palette memory: generator not landed); 256
unvalidated levels would break the level-count law and couple this stream to a
palette that doesn't exist. When the palette lands, the authored colors can
become palette references without touching the trait shape.

## F5 — ONE merged-traits accessor; constant derives, runtime rides payload {#f5}
_2026-08-08 · resolved at plan time_

**Chosen.** A single shared/content accessor answers "what traits does this
object carry": the def's CONSTANT assignments (derived, zero storage) merged
with the payload's runtime rows (pawns, exactly today's lane). `mint_sidecars`
stops minting rows for constant binds. All readers — stat_eval, emotions,
affordance Tag checks, inventory capacity, the client panels — move to the
accessor; it is also the ENFORCEMENT point: a write path targeting a constant
assignment refuses loudly ([I4](issues.md#i4)). On a name collision the
CONSTANT assignment wins and the runtime row is ignored (stated, simplest).

**Why.** The ONE-eval law (needs-moodlets) already forces shared logic; a
second ad-hoc "def traits + payload traits" merge per consumer is exactly the
drift that law exists to prevent.

## F6 — things accept CONSTANT assignments only (v1) {#f6}
_2026-08-08 · resolved at plan time_

**Chosen.** Load validation refuses a non-constant trait bind on a non-pawn
thing. Runtime trait attachment to THINGS is a NAMED SUCCESSOR (it needs a
thing-side gameplay sub-table that does not exist).

**Why.** The user's own constraint: a thing carrying non-constant traits
cannot be saved to cold — and cold is where things live. Refusing at load is
the honest v1; silently accepting-and-losing rows on the next cold save would
be the worst outcome. Pawns keep both flavors (payload storage exists).

## F7 — `visual.light` DELETES; `thing_light()` derives from the trait {#f7}
_2026-08-08 · resolved at plan time_

**Chosen.** Both torches convert to constant `emit_light` binds; the `light =`
block and its `LightToml` lane die in the same change (delete, don't
deprecate). `thing_light()` keeps its stride-8 output shape but composes it
from the def's constant emit_light + the trait def's presentation residue —
the client's `lightFor(kindId)` and the whole cold-baking path stay untouched
([I3](issues.md#i3)).

**Why.** Two authoring lanes for one phenomenon is how the corpus rots; the
consumer shape staying constant makes the deletion cheap.
