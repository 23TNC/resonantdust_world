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

**Pawn defs become real packed defs.** (User, 2026-07-30-b: "hold pawns as
type/subtype/kind and populate it from there.") A pawn's stored `definition_reference`
stops being a raw content `object_id` and becomes the packed form:
`TYPE_PAWN | subtype (species: animal/human) | kind (wolf/female/male) | variant:4`.
The easy case is served by the def alone — the wolf's sprite variant is `def.variant_id`.
`data:u8` (`facing:2 | serial:6`) is untouched: movement state stays movement state.

**The payload — a growable opcode stream, like the command buffer.** (User, 2026-07-30-b.)
Pawns will accrue inventory, stats, needs, memories… so per-pawn state generalizes NOW
instead of adding a nibble per feature: a pawn carries a **`payload: Vec<u32>`**, encoded
like the event command buffer — a header word `opcode:16 | count:16` followed by `count`
operand words, entries concatenated, so the payload grows/shrinks per pawn. First opcode:
**`PART` (count 2: `slot`, `definition_reference`)** — the FULL def the part slot draws.
The human carries `PART(0, body def)` + `PART(1, head def)` so either swaps when armor is
equipped (an armor def simply replaces the slot's def — a future equip verb rewrites one
entry); a wolf carries no entries and draws its own def. A future inventory is just
another opcode + count. `TABLES.md` owns the encoding.

**The payload lives in SIDECAR tables, SLAVED to state.** (User, 2026-07-30-d.) Putting
the vec on the entity rows would make every movement hop copy it forward — cheap at two
`PART` entries, ruinous at a full inventory. Instead the pawn shard holds two more tables,
**`payload_log`** + **`payload`**, mirroring the log/composed split of the entity pair;
the generic rows stay untouched, so **hops cost nothing** and payloads can grow complex.
The cross-table hazard (state locked while payload edits, or vice versa — an action
touching one while the other is claimed breaks invariants) is resolved by **slaving
payload to state**: the payload is NEVER claimed independently — the entity's existing
state claim IS the payload's lock, and every payload write happens inside the same
transaction as a state write (spawn now; equip later). The existing lock machinery
continues functioning unmodified. Consequences the slaving implies: the `payload` row
carries the zone key so the zone subscription fans it — a ZONE-CROSSING state write
re-keys it (the only movement that touches payload, and only its key); entity removal/gc
cleans the sidecar rows under the same claim.

**How the payload arrives — and `CREATE` is THE creation verb.** (User, 2026-07-30-c: one
existing verb creates essentially any object — a new pawn, a stack of logs — no new
action.) `CREATE` becomes variable-arity (the `INIT_ZONE` precedent):
`def · position · count · payload×count` — the spawn transaction is the one place the full
first row is known (`SET` targets cold rows only; nothing else can address a minted id).
With the def now packed, **`def_type_id` is `CREATE`'s routing key**: the worker routes
the mint to the type's shard. This stream implements the `TYPE_PAWN` arm only; any other
type is rejected with a named error, NOT silently pawned — so the future arms (e.g.
`TYPE_THING`: a minted log stack) slot in per type without reshaping the verb. The
distinction from `SET` stays crisp: `CREATE` mints an IDENTITY + first row; `SET` writes a
CELL (terrain/walls — no identity). The worker's row composition (MOVE_TO/MOVE_STEP/PLACE)
**carries the payload through unchanged** — movement verbs never touch it. `ACTIONS.md`
owns the signature.

**How variant + part art is addressed.** The go-forward texture tree is
`<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>` (already the human tree's
shape). The stem grammar grows to address it: `<base>/<variant>/<facing>.<part>`, with
variant defaulting to canonical `1` and part to `0` — every existing stem keeps resolving
unchanged. Each (variant, facing, part) is its own served master → its own immutable defs;
stems are fetched on demand, so 16×3×2 potential leaves cost nothing until a pawn wears them.

**The parts model (DSL = structure, payload = wardrobe).** The VM already appends one prim
per `^prim call`; only the loader stops at `prims.0`. The loader learns to read **all**
prims as a parts list; a part gains `&prim.part` (which `<part>` files it draws, default
0), `&prim.scale` (size multiplier against part 0's size, default 1 — the head authors
0.625), and `&prim.offset.x/y` (tiles, relative to part 0's anchor). The DSL kind declares
the SKELETON — slots, scales, offsets; the payload's `PART(slot, def)` entries supply WHAT
each slot draws (stem + variant resolved from that def). A slot with no payload entry
draws the pawn's own def (the wolf's whole model). MoverLayer renders every pawn as its
parts list — the wolf is the 1-part degenerate case, humans are 2.

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
- Movement hops must NOT touch the payload sidecar (that is the point) — except a
  zone-crossing write re-keying the `payload` row's zone under the same claim; verified
  by a live trip (with a crossing) over a non-empty payload.
- The def repack touches every consumer of the pawn's `definition_reference` (the
  worker's hop-cost lookup, the npc's resolve, the client's kind decode, the wolves
  brain's adoption filter) — each moves from object_id to `def_kind_id`/full-def compare
  in the same phase, or the wolf stops resolving.
- Wolf behaviour must not regress: same walk, same look (canonical art), variant now from
  `def.variant_id` instead of hashed from the id.

## Out of scope

Hand parts and equip verbs (the payload's `PART` shape is BUILT FOR them — an equip
rewrites one slot's def — but no verb work now); inventory/stats/needs opcodes (the
opcode stream is the extension point; only `PART` is defined here); wolf multi-variant
art normalization (its variant folders are sprite-gen outputs, not a clean 0..N — the
wolf keeps minting variant 0 = canonical art); character-generation UI (the
fit/fat/average grouping is recorded here + in the DSL comments for that future).
