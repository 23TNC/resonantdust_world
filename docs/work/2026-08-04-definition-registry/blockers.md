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
