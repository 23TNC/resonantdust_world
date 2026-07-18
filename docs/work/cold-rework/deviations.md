# Deviations — cold-rework

_Plan deviations, logged at the moment of deviating. What the plan said, what we did, why._

---

## D-1 · Cold entity id is **deterministic-from-position**, not a minted counter — collapsing the two-phase

> **Superseded by F6 (2026-07-18).** The `overlay`-is-`sparse!` model drops per-cell
> `entity_reference`s entirely — a cold cell is addressed by its biome-row (`cold_row_reference`) + a
> `tile_reference` within it, no `cold_entity_reference` and no per-cell id. Kept for the reasoning
> (why a deterministic-from-position key beat a minted counter — the same instinct the row-addressing
> now embodies). The built `SET`-to-`state` path from this deviation is what `overlay` replaces.

**Plan (intent §"The mutation lifecycle" + §"Core owns the two-phase").** `UNPACK` *mints* an
`entity_reference` (a server counter), and core learns the id by **watching the position in `state`**
across tics (N=0 pre-empt → ~N=3 the row appears → issue the op against the resolved id). The
worker/orchestrator gain a **deferred spawn-id claim** so a mint gets a claimable slot.

**What we did.** A cold cell's identity *is* its position ("one thing per cell, so position
disambiguates" — intent). So its `entity_reference` is a **pure function of position**:
`cold_entity_reference(server_reference, position) = server_reference:8 | macro_position:16 |
tile_reference:8` (the `u24 object_reference` holds `macro:16 | tile:8` exactly; layer is the shard's).
The caller **computes** the id and spells it out as an ordinary `Write` operand of a new `SET` action.

**Why.** This is still "server-minted, deterministic-from-event" (the id is a deterministic function of
the event's position operand) — but because it's deterministic *and position-derived*, **the caller
already knows it**. So:
- **No deferred spawn-id claim.** The write target is present at grouping time (like every other
  action), so union-find groups it and the orchestrator claims its slot — the existing machinery, no
  new mint-time path in the most correctness-critical code.
- **No position→id watch in core.** Core computes the id locally; the two-phase collapses to a single
  op. (The watch was only needed because a *counter* mint's id is unknowable until it lands.)
- **Idempotent by cell.** Re-issuing `SET` for a cell reuses the same id — matches "one entity per
  cell" and the old `set_tile` reuse-by-position, without a `mint_counter`.

This mirrors the existing `wolf_key` stopgap (client computes a deterministic id and spells it out),
except the cold id is derived from *position* rather than an index — so it's not a stopgap, it's the
cell's true identity.

**Consequence.** The general free-counter `CREATE` (a *pawn* spawn, whose identity is **not** its
position) still needs the deferred spawn-id claim — that remains future work, unblocked by this. The
`mint_counter` in `tile`/`thing` is retired; the direct `set_tile`/`set_thing` reducers adopt the same
`cold_entity_reference` so both paths address a cell identically. The worker + orchestrator route a
target's `write`/`claim` to its shard by `entity_ref_server_reference` (top byte) — additive; a pawn's
`0x30` still falls through to `data_shard` unchanged.
