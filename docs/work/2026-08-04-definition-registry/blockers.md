# Blockers — definition registry

_Things that genuinely need the user. A decision I can make is a [fork](forks.md), not a blocker.
Every row here is a call whose answer changes the SCHEMA or the WIRE LAYOUT — get one wrong and the
cost is stored data, not a refactor._

## B2 — `variant_id` is u4 and one kind already holds 15 of 16 (OPEN) {#b2}

_Opened 2026-08-04; narrowed the same day once [F10](forks.md#f10) settled subtype. Blocks P1
(the schema). Irreversible once ids are stored._

`pawn/animal/wolf` has **15 variants today** against a `variant_id:4` field with 16 slots
([I3](issues.md#i3)). The labels are fine — they live as `string variant` in the registry row, and
the id carries only the slot. The **count** is the problem, and it is live rather than theoretical.

My earlier framing offered a free fix — drop the "unused" subtype half. [F10](forks.md#f10) killed
that, correctly, so widening variant now means **narrowing another field**, and every option is a
stored-data layout change:

| Option | Layout | Buys | Costs |
|---|---|---|---|
| leave it | `type:4 \| subtype:12 \| kind:12 \| variant:4` | nothing moves | one more wolf variant and we are stuck |
| narrow subtype | `type:4 \| subtype:8 \| kind:12 \| variant:8` | 256 variants, kind untouched | 256 subtypes (biomes + species) instead of 4096; `type_reference` (the u16 cold-row header) changes shape |
| narrow both | `type:4 \| subtype:8 \| kind:12 \| variant:8` … or `type:3` | more headroom | `type:3` caps types at 8 and they are structural |
| widen the word | `u64 definition_reference` | everything | every stored def, every packed record lane, the u16 half-split |

**My recommendation: narrow subtype to u8, widen variant to u8.** Versioning burns *kind*
([F5](forks.md#f5)), so kind is the field that must stay big — subtype only ever holds real biomes
and species, where 256 is generous and 4096 is speculative. It keeps the 32-bit word and the u16
half-split, and it quadruples the axis that is actually full.

**Why it needs you**: it changes `type_reference = type_id:4 | subtype_id:12`, the *shared* cold-row
header, not just the def word. That is a wire layout with stored data behind it, and P0's third item
exists to cost exactly this before it is chosen.

## B1 — the four-segment stems and `white` — ✅ RESOLVED (withdrawn, my error) {#b1}

_2026-08-04. Not a question; I misread the corpus._

`biome-tile/default/smooth/wall` is `type=biome-tile, subType=default, kind=smooth, variant=wall`.
The taxonomy is uniform four axes; the corpus `texture` field is just the `type/subType/kind`
**prefix**, with variant and direction appended at resolve time — which is why the live manifest
holds `biome-thing/default/conifer/0/e` where the corpus holds `biome-thing/default/conifer`.

`white` is the built-in no-art fill, not a taxon: no `white` file exists under `textures/`, and the
loader, the client and `def_span.py` all special-case the string. My proposed `art = none` was a
rename of what already exists. See [I4](issues.md#i4).

## B3 — what bumps a version — ✅ RESOLVED {#b3}

_2026-08-04, the user: "Fine."_ Bump on a change to any field the **simulation** reads; never on
art, tint, or comments. Recorded as [F12](forks.md#f12) — the invariant it buys is
**"same id ⇒ same behaviour"**, which is what [F6](forks.md#f6)'s old-apple policy actually needs.

## B4 — where the registry lives, and who allocates — ✅ RESOLVED {#b4}

_2026-08-04, the user: "master allocates as it is a single master. We can use index."_

Recorded as [F11](forks.md#f11). The registry table goes in the per-env `index` DB; `server/master`
owns allocation, and being singular it removes the boot race I was designing around. My objection —
that `index` is a routing directory — was rebutted on the facts: the module split is a **scaling**
boundary (data shards grow independently), not a lifecycle division. A sibling `definitions` module
stays available, exactly as `chat`/`players` sit beside `index`, if reclaim ever needs its own
reducers.
