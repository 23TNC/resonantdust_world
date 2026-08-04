# Completed — definition registry

_Dated evidence: what landed and how it was checked. Append chronologically._

_Nothing yet — the stream is planned, not started. P1 and P2 are gated on
[`blockers.md`](blockers.md)._

## 2026-08-04 · P0 — freeze what a def id means today (3/3)

**The inventory turned up the finding that reshapes P4** ([I6](issues.md#i6)). Seven sites touch a
`definition_reference` — three pack, four unpack — and no site outside the pre-planning measurement
needed investigating. But look at what the two relay sites compose from
([`ws.rs:479`](../../../server/edge/src/ws.rs), `:508`): a code constant, the **cold row's own
`subtype_id`**, and the **stored `kind_reference`**. The def is assembled from fields already in the
row; no name is resolved and no table is consulted.

So the registry is needed at exactly two moments — allocation, and name→id resolution. Not on the
read path, not on the relay path, not on the render path. P4 re-points resolution only, and every
stored id keeps decoding exactly as it does today.

**The oracle split along the crate boundary**, because `shared/content` deliberately has no codec
dependency and adding one for a fixture would couple them:

- `name → kind_id` for tiles and things, as explicit pairs in the golden corpus (blessed; the
  registries already pinned it *implicitly* as line numbers, which is a reading convention, not an
  assertion).
- the packed composition in a new `npc::def_fixture` module — where codec lives, and where the
  composition actually happens. `wolf → 0x30010070` (the value the live npc logs on every boot),
  `human_female → 0x300200A0`, `human_male → 0x300200B0`, with speeds.

The second `def_fixture` test deliberately asserts the coupling P5 will **remove** — that a pawn's
species nibble equals `pawn_species_subtype_id` of the texture stem's second segment. Pinning it now
makes its removal a visible, deliberate change rather than a silent one; a re-subtyped pawn would be
adopted and rendered wrong forever.

**Stored ids** ([I7](issues.md#i7)): 5 stores in two shapes. Tiles and things keep only the KIND
HALF (`kind_id:12 | variant_id:4`) — their type is implied by the module and their subtype comes from
the row header. Pawns store the WHOLE def, in the `pawn` table and again in the payload's `PART`
entries. **Nothing stores a name**, which is what makes [F6](forks.md#f6) free: a stored row already
means what it meant, and the registry never has to rewrite one.

Verified: 12 content tests + the blessed golden green in the shared workspace; both `def_fixture`
tests green in `client/npc`.
