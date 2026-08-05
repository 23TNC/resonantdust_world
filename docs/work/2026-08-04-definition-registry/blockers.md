# Blockers — definition registry

_Things that genuinely need the user. A decision I can make is a [fork](forks.md), not a blocker._

**All resolved as of 2026-08-04 — the stream is unblocked.**

## B2 — the packed layout — ✅ RESOLVED: it does not change {#b2}

_2026-08-04, the user, after I raised it twice._

> "We are not changing our data structures. I stuffed a u4 into a u8 in our table because we cannot
> assign a u4 table. That's it."

`definition_reference` keeps `type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4`, and
`type_reference` keeps `type_id:4 | subtype_id:12`. **No field widens, narrows, or moves.** The u8
column in the table is a storage artifact — SpacetimeDB has no u4 — and carries no design signal.

The consequence, recorded as a constraint rather than a problem: **16 variants per
(type, subType, kind)**. `pawn/animal/wolf` holds 15 ([I3](issues.md#i3)), so it has one slot left.
That is an authoring limit the allocator enforces loudly ([P2](todo.md)); a kind that needs more art
splits into more kinds. This stream does not touch the wire.

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

## B5 — `kind_id` serves two masters, and versioning breaks their equivalence (OPEN) {#b5}

_2026-08-05, P6. Found by the live apple drill, not by any test._

The wolf bumped to v2 and the npc resolved it correctly — `def=0x300100d0`, `max(version)` working
exactly as designed. Then it reported **`speed=3 thirst_need=0`** instead of `speed=12 thirst=1`.

`kind_id` in the packed def is doing **two different jobs**, and until a version bump they happened
to be the same number:

1. **Stored identity** — what a zone's `kind_reference` means, what the registry allocates and never
   reuses. A bump MUST take a fresh one ([F5](forks.md#f5)), or two versions share an id and the
   table's primary key rejects the second.
2. **An index into the corpus's per-def tables** — `thing_speed`, `thing_needs`, `thing_layout`,
   `visual_for_object`, the render decode's `def_kind_id(...)` lookup. These are `Vec`s in corpus
   order; the index must be within range.

v2's `kind_id` is **13**. The corpus has **11** things. So the identity is right and every table
lookup falls off the end into defaults — speed 3 is `speed::resolve(None)`, thirst 0 is "no needs".

I recorded the opposite in [I1](issues.md#i1) — *"the render decode uses it as an opaque table
index, which is precisely why F5 can burn kind ids without touching a consumer"*. That was wrong:
an opaque index still has to be **in range**, and F5 guarantees it eventually is not.

**Why it needs you.** Every fix changes what the render path indexes by, and that path is hot
(per cell, per zone) and touches stored data:

1. **Split the two roles.** `thing_object_id(name)` returns the corpus POSITION (table index);
   `definition_reference(name)` returns the registry id (identity). Worldgen packs the latter into
   zones; the render decode maps a stored `kind_id` back to a position through the registry. Correct
   and explicit — but it puts a lookup on the per-cell decode, which [I1](issues.md#i1) says is the
   one place that cannot take one.
2. **The Bundle indexes by registry `kind_id`** — build the per-def tables sparse/keyed rather than
   positional, so index 13 is simply the v2 wolf. No lookup on the hot path; costs a table rebuild
   and makes the tables sparse.
3. **A bump reuses its `kind_id` and versions elsewhere** — impossible while `id` is the primary key
   and the packed layout is frozen ([F13](forks.md#f13)), unless version rides a field it currently
   does not.

**My recommendation: option 2.** It keeps the hot path a direct index, which is the constraint that
actually binds, and the sparseness costs nothing at this scale (a few thousand slots). Option 1 is
cleaner on paper and I would take it if the render decode were not per-cell.

**State right now**: the dev registry holds wolf v0/v1/v2 from the drill and `max(version)` resolves
to v2, so the npc reads default speed. Wiping and re-seeding `index` returns dev to one clean row
per tuple — the drill's rows are real history, not corruption, but they are not worth keeping.
P6 items 1–2 are otherwise verified (a data change bumps, an art change does not, old rows survive);
item 3 — the two versions coexisting in the client — is what this blocks.
