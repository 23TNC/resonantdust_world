# 005 — Cold `find-or-mint` can't decode into a generic payload (phase S6/S7)

## Problem

`find-or-mint` (promote a cold object to hot when a row targets it) must **write the payload
fields** of the new hot entity — its `kind`, `zone_id`, `location`, … taken from the cold
object. But `decl_tick_pipeline!` is payload-**generic**: inside the macro the payload is an
opaque `$( $pf : $pt )*` list, so the module *cannot* map "the cold object's kind" onto "the
`kind` field." (This is the same reason the old `unpack` reducer took the whole payload as
args — a reducer can't decode a cold object into a payload it doesn't understand.)

So `find-or-mint` can't live wholly inside a module reducer.

## Options

- **A · Worker decodes; module gets a payload-parameterised `mint_cold`.** The worker (which
  subscribes `cold` and *is* the spatial resolver) does the decode: on a **positional/cold
  target**, it looks up `state` for an existing hot entity at that location; if none, it reads
  the `cold` object, decodes its kind/position, and calls a macro-emitted
  `mint_cold(event_ref, positional_target, <payload…>)` reducer that seeds the hot entity +
  appends the `cold_removed` tombstone. Keeps the module generic; the decode is where the
  payload knowledge already is.
- **B · Payload-specific mint in the module** — break genericity: a spatial-only `mint_cold`
  hard-coding `kind`/`zone_id`/`location`. Simple but couples the generic engine to one payload.
- **C · Interpreter-side** — the mint stores a raw `object_kind_reference`, and the action's
  program decodes it into state. Pushes cold decoding into every plan; heavy.

## Choice — **A**

Worker decodes a positional/cold target and calls a payload-parameterised `mint_cold`.

## Why

- **A matches the design** ([hot-cold.md](../spacetime-tables/hot-cold.md), decision that the
  worker does the game-logic decode; the module is a store) and keeps `decl_tick_pipeline!`
  payload-generic — the `mint_cold` reducer is emitted with `$pf` like `resolve`/`seed_entity`.
- **B** re-couples the engine to the spatial payload, undoing the generalization.
- **C** spreads cold-object decoding across the DSL for no benefit.

## Scope (the remaining S7 find-or-mint work)

1. Module: emit `mint_cold(event_ref, positional_target, <payload…>)` — mint a hot key, seed its
   resolved state from the args, append the `cold_removed` tombstone, and open the action's
   pending row against the minted key.
2. Worker: on a **positional** target (`entity_ref_is_positional`), check `state` by location
   (existing hot → reuse); else read `cold` + decode the kind at `(x,y)` and call `mint_cold`.
   (Worker subscribes `cold`.)
3. Edge: `interact` targets the cold object by a **positional** `entity_reference`
   (`pack_positional_entity(zone, location)`); enqueue promotes it.
4. Live-test the cold→hot promotion (interact on a seeded cold thing).

## Status

Design resolved; implementation is the next phase. Blocked nothing already shipped — the hot
path is fully live-verified; only cold `interact` remains stubbed.
