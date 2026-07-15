# Intent — the reference model (why what/where/state are separate)

_The why behind [`../design/reference-model.md`](../design/reference-model.md). Last updated: 2026-07-14._

## Why three references, not one packed identity

v1 packed *what* + *where* + *state* into a single `u64 object_reference`. That conflated a
**shared blueprint** with **per-instance** facts: two oak trees on different tiles had different
"object_references," so the thing that should be shared (their definition) wasn't. Splitting into
**definition** (what) / **position** (where) / **data** (state) makes each name honest and each
concern independently addressable — you can talk about "an oak" without saying where it is.

## Why `u32` everywhere (and the cost)

Every reference is a fixed-width packed integer so references **compose**: a `u32
definition_reference` / `hot_reference` / `cold` address / `event_reference` all drop into a `u64
action_reference` (`server_reference:16 | payload:32`), so an action carrying any of them is passed
identically — no per-kind special-casing (see [`event-reference.md`](../../../server/spacetime/modules/shard/intent/event-reference.md)).
The cost we accept: `definition_reference` is a **fully-packed u32 with no headroom** — that's why
`subkind` was dropped and `kind` capped at 4096. Density-for-composability, deliberately.

## Why drop `subkind`

We were burning `u4` on a level we didn't need. Flattening `kind` + `subkind` into one `kind_id`
(`male.fat` is one kind, not `male` × `fat`) costs the ability to vary those axes independently —
accepted, because enumerating combined kinds is fine and it's what buys the `u32` definition.

## Why the cold row hoists the shared bits

Cold's whole purpose is **compact data to cut wire overhead** — SpacetimeDB re-transmits a whole
row to every subscriber, so a zone full of one subtype's things wants a tiny per-object cost. So
`(type, subtype, region, zone, layer)` live once in the row header, and each object is just
`kind | tile | data` = 32 bits. `data`'s decode is picked by the row's `type_id`, read once per
type-homogeneous row.

## Why `data:8` defines "packable" (the cold/hot boundary)

`data:8` is the **pack criterion**, not a limitation. Cold serves the mass common case; an object
whose state doesn't fit stays **hot** — full id + a `state` row + a bit more wire per object. The
pack action skips objects too state-dense to compress (pawns, with inventories/needs, never pack;
dirt always does). This keeps us from ever widening the per-tile cost to serve the stateful
minority — they're hot by definition, and there's no reason to pay for their state on every tile.

## Why `object_reference` is one universal handle (+ `reference_id`)

Everything is an object, so one `u32 object_reference` addresses any of them — `hot` / `cold` /
`position` / `event` / `server`. Most of the time the **container disambiguates** the variant (the
`state` table ⇒ hot, `cold` table ⇒ cold, `event_log` ⇒ event; a DSL slot ⇒ per the action), so no
tag is spent on the wire. `cold_reference` and `position_reference` are kept **separate** (same
bits, different type) precisely so "target the tile" vs "target the cold object at the tile" is
never ambiguous — one is `unpack`able, the other isn't. When you genuinely need disambiguation
standalone, you carry the full `entity_reference`, whose `reference_id:6` names the variant — so we
pay for the tag only when we need it, never per operand.

## Ties into

Settling this closes the **taxonomy/naming** piece and the identity/keying divergences — all
**done + browser-verified** (2026-07-14): the `hot_reference` re-key (**#5**, via the
`entity_reference` layout), the geographic `server_reference` (**#10**), and the cold row's
`macro_position_reference` + `layer_id` header (**#4** / **#11** — see
[`work/…/deviations.md`](../../../../work/spacetime-rewrite/deviations.md) D-3, the deviation that
produced it). Blockers [B-1](../../../../work/spacetime-rewrite/blockers.md) + B-2 are both
resolved.
