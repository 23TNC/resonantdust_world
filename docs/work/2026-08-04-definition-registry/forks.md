# Forks — definition registry

_Decisions taken in the design conversation of 2026-08-04, recorded before any code._

## F1 — the corpus authors TAXONOMY, not ids {#f1}

A definition authors `type` / `subType` / `kind` / `variant` as **names**. It authors no numbers.

**Chosen** because the four coordinates already exist in the wire id and are currently recovered by
parsing an art path through a code-owned palette — the definition's own identity derived from where
its pictures live. Authoring them inverts that: the stem becomes derivable from the taxonomy
(`<type>/<subType>/<kind>/<variant>` is already the go-forward texture path shape in VARIABLES).

**This SUPERSEDES [`toml-content` F1](../2026-08-04-toml-content/forks.md#f1)** (every def authors
`id = N`; the loader refuses duplicates/zero/missing). That fork was correct while the *loader* owned
identity — ids were stored data and TOML table order was too fragile to carry them. A server-owned
registry removes the premise, and the earlier fork's own reasoning points here: it noted a content
registry could own these numbers "once it guarantees append-only numbering."

Rejected: **keeping explicit ids alongside the taxonomy** — two places describing one identity, and
the numbers would have to be hand-kept in sync with a cross-product the server computes.

## F2 — a definition is (type, kind) + APPLICABILITY ARRAYS {#f2}

`subType` and `variant` are **arrays** on the definition; `type` and `kind` are scalars. The
definition applies to every tuple in the cross-product.

```toml
[[thing]]
type    = "biome-thing"
kind    = "conifer"
subType = ["forest", "plains", "grassland"]
variant = [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]
```

One block, 48 tuples. A wall that exists in exactly one form still pins single-element arrays, so
there is one spelling, not two. At load the server expands the cross-product and allocates a row per
tuple — **the expansion is the allocation step**.

Rejected: **one block per tuple** (16 conifers because there are 16 tree variations — the thing the
user named first); **a wildcard/glob** (`subType = "*"` reads as "all present and future", which
would silently capture a biome added later and mint ids nobody authored).

## F3 — the id stays STRUCTURED; `version` is a column, never a field {#f3}

`definition_reference` keeps `type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4`. The registry
does not make it an opaque handle.

**Forced, not chosen** — measured in [I1](issues.md#i1): three hot paths unpack the bits with no
table available, and the worker's `CREATE` routing is a safety property that runs inside the sim per
event. An opaque id would put a registry lookup on all three.

Consequence: a version bump cannot be "same id, higher version". It **allocates a fresh coordinate**,
and the `version` column exists so a *name* lookup can find the newest — not so an id can carry one.

## F4 — ONE version column, bumped on any change {#f4}

`u32 version` per row. Any change to the definition bumps it, minting a new row with the same four
strings and a new id. Resolution: match the four strings, take `max(version)`.

Rejected: **per-axis versions** (`subTypeVersion` + `kindVersion`, the first sketch) — two columns to
express one fact, and it invites versioning a subType, which [F5](#f5) rules out anyway.

Open: **what counts as a change** — see [B3](blockers.md#b3). Data-only, or art too?

## F5 — a version bump burns KIND id space, not subType {#f5}

`conifer` v1 gets a new `kind_id`; `forest` keeps its `subtype_id`.

**Chosen for isolation, not cost.** Versioning by subType (`forest.1`) would version *every kind in
that biome* at once — a conifer edit silently re-ids the rocks. Versioning by kind confines the
change to the thing that changed.

That `kind_id` is read on a hot path is **not** an obstacle: the render decode treats it as an opaque
index into the visual tables ([`wasm/lib.rs:523`](../../../shared/wasm/src/lib.rs)), so two
`kind_id`s both meaning "conifer" are invisible to it.

Rejected: **burning subType** (above); **burning variant** (u4 — 16 total, and the wolf already
holds 15, see [I3](issues.md#i3)).

## F6 — old objects keep old ids FOREVER; no migration sweep {#f6}

Highest-version resolution affects **new placements only**. An existing entity holding a v0 id keeps
it and keeps behaving as v0 until it is destroyed or spent.

**The user's call, and the reason this design exists** — a sweep has to answer questions that have no
good answer: new apples expire in 1 h and old ones in 2 h, so what happens to an apple with 30
minutes left? Apple weight goes 1 → 2, so what happens to inventories now over capacity? "If all old
apples are old apples and function like old apples, I don't think we would have many issues."

Rejected: **sweep-on-update** (the edge cases above, each needing a bespoke rule per field per
version); **refuse the update until nothing references the old version** (blocks content work on
world state).

Consequence to accept openly: the world holds a **mix of versions indefinitely**, and that is
correct. Anything that assumes "all apples behave alike" is already wrong and this makes it visible.

## F7 — reclaim is DESIGNED, not BUILT {#f7}

The user's GC sketch — periodically check every shard for an obsolete id, and once it is gone
everywhere, reclaim the coordinate for something else — is written into the plan and deliberately
left unbuilt.

**Chosen** because reclaim is the one irreversible step, and "gone from all shards" is not the same
as "gone". `entity_state_log` is append-only **history**: an id reclaimed while an old event still
names it does not merely leak, it makes replay *lie* — `0x01020304` was a conifer when the event was
written and is moss when it is read. Same hazard for offline shards, backups, and a client holding a
stale registry.

u12 `kind` is 4096 coordinates. Burning one per kind per version is a very long runway, and the cost
of not reclaiming is a number we have plenty of.

## F8 — direction stays OUT of the id {#f8}

Confirmed against the code, not assumed: direction lives in the texture filename
(`albedo.<dir>.<part>.png`) and the record's rotation lane, and `data_rotation` is a separate
operand. A lamp is id N facing any way — direction does not define the object.

## F9 — clients resolve `string → id` LOCALLY, from the served registry {#f9}

The registry table is pushed to clients on load and on change; lookup is local. No round-trip to
resolve a name, and no names on the action wire (there are none today — every verb operand is already
a packed reference; the only wire strings are `Login`/`SetAnchor` player-facing labels).

A **stale client resolves a name to an older version** and places an old-version object. Accepted:
that is exactly what [F6](#f6) makes safe, and it self-corrects on the next table update.
