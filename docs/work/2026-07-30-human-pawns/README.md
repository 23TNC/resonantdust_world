# human-pawns — extend the pawn class: multi-part pawns, persistent variants, the first human

## What (user, 2026-07-30)

Pawns generalize to **multi-part objects**. The wolf needs one part; a human needs **body
(part 0) + head (part 1)** and will eventually gain hand parts. A human pawn stays a
**single object** like the wolf — one entity, one DSL kind — with its primitives declared
in the human's DSL. Pawns hold **u4 variants for body and head in spacetime** (they are
what makes up that pawn); the wolf must also retain its variant (today it does not — the
client derives one from the entity id and always receives canonical variant-1 art). No
bespoke classes: the wolf renders through the same parts machinery as humans, with one part.

The user's content spec:

- **Bodies** (per sex, 9 variants each, art already mastered at
  `textures/pawn/human/{female,male}/{0..8}/<map>.<dir>.0.png`):
  fit = variants **0, 1, 2** · fat = variants **3, 4, 5** · average = variants **6, 7, 8**.
- **Heads** (per sex, 16 variants, art at `…/{0..15}/<map>.<dir>.1.png`): any head applies
  to any body of the same sex. Head canvases are held about body size, so the head prim
  carries **scale = 0.625**.
- Naming: the convention is **`part`** (`<map>.<dir>.<part>.<ext>` — the manifest's
  `&layers` wording is the legacy term).

## Design stances

**Where the variants live.** The pawn shard's `data` is a u8 of `facing:2 | serial:6` —
fully occupied (MOVE_STEP chain supersession reads the serial). And a pawn's stored
`definition_reference` is today a raw content `object_id`, not a packed
`definition_reference`, so its variant nibble does not exist on the wire either. The shard
widens: `entity_tables!(data: u16)` for the pawn module only, layout
**`body_variant:4 | head_variant:4 | facing:2 | serial:6`** — the low byte stays
bit-identical to today (every existing decoder keeps working), the high byte is new. When
the object-model migration packs pawn defs, body_variant may move into `def.variant_id`;
until then both nibbles ride `data`. `TABLES.md` owns the shape.

**How variants arrive.** `CREATE` grows a third immediate: `data` (arity 2 → 3) —
`def · position · data`. The spawn reducer writes it into the first row. `SET` cannot do
this (it targets cold rows only), and a follow-up verb on the minted id has no client-known
target. `ACTIONS.md` owns the signature.

**How variant + part art is addressed.** The go-forward texture tree is
`<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>` (already the human tree's
shape). The stem grammar grows to address it: `<base>/<variant>/<facing>.<part>`, with
variant defaulting to canonical `1` and part to `0` — every existing stem keeps resolving
unchanged. Each (variant, facing, part) is its own served master → its own immutable defs;
stems are fetched on demand, so 16×3×2 potential leaves cost nothing until a pawn wears them.

**The parts model (DSL → client).** The VM already appends one prim per `^prim call`; only
the loader stops at `prims.0`. The loader learns to read **all** prims as a parts list; a
part gains `&prim.part` (which `<part>` files it draws, default 0), `&prim.scale`
(size multiplier against part 0's size, default 1 — the head authors 0.625), and
`&prim.offset.x/y` (tiles, relative to part 0's anchor). Which STATE nibble feeds a part's
variant follows the part index: part 0 ⇒ body_variant, part 1 ⇒ head_variant. MoverLayer
renders every pawn as its parts list — the wolf is the 1-part degenerate case, humans are 2.

**Single object, single carrier.** One entity, one DSL kind, one warm carrier. The final
phase conforms the records to the documented primitive graph (`VARIABLES.md`: "a pawn =
prim{head, body, hand-prim, hand-prim}"): the head billboard attaches as a carried piece of
the body's carrier prim with authored offsets, the same slot machinery lights already use.
P0–P4 deliver the user-visible feature with per-part records; P5 is the graph conformance.

**The first human doubles as the drill fixture.** The npc spawns ONE static human at a
known location and never moves it — exactly the standing A/B fixture the user asked for
during shadow-polish ("make another pawn that the npc doesn't move at a static known
location").

## Constraints

- Append-only content ids: `human_female` / `human_male` append after `torch_blue`.
- The pawn schema change is live: docker redeploy + a LIVE subscription-SQL check
  (subscription SQL is a string — the build gates cannot catch it).
- MOVE_TO/MOVE_STEP must compose the serial in the LOW byte only, preserving the variant
  byte — a masked read-modify-write, verified by a live trip with a nonzero variant byte.
- Wolf behaviour must not regress: same walk, same look (canonical art), variant now from
  state instead of hashed from the id.

## Out of scope

Hand parts (the parts model must not cap below 4 pieces, but no hand work now); wolf
multi-variant art normalization (its variant folders are sprite-gen outputs, not a clean
0..N — the wolf keeps minting variant 0 = canonical art); character-generation UI (the
fit/fat/average grouping is recorded here + in the DSL comments for that future);
migrating pawn defs to packed `definition_reference`s (recorded above as the eventual home
for body_variant).
