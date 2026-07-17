# Deviations — coord-purge

_Logged at the moment of deviating, per convention. A deviation is a departure from the item's written
plan, with the reason._

---

## D-1 · `macro_to_zone_id` survives item F (deleted in G instead) — RESOLVED in G

**Status:** Resolved. Item G re-keyed the anchor manager to macro, so the engine passes the wire macro
straight into `note_update`; `macro_to_zone_id` and `zone_id_to_macro` were both deleted in G. The
render invariant F targeted held throughout.


**Planned (todo F):** "Delete `world.rs::macro_to_zone_id` (its only purpose was reconstructing
`zone_id` for the render)."

**What happened:** `macro_to_zone_id` has a *second* caller the plan overlooked — the engine loop
(`engine.rs` + `web.rs`) converts the wire macro back to a `zone_id` to call
`self.zones.note_update(zone_id, …)`, because **the anchor manager (`zones.rs`) still keys its
subscription bookkeeping on `zone_id`**. That is exactly item G's rework. So F could remove
`macro_to_zone_id` from the *render* path (the `Event` variants now carry `macro_position` straight
through) but **not** delete the function — `note_update` needs it until G re-keys the anchor manager to
macro.

**Resolution:** `macro_to_zone_id` stays after F, used *only* by the anchor `note_update` bridge (each
call site now carries a comment saying so). Item G deletes it together with `zone_id_to_macro` when the
anchor manager speaks macro. F's real invariant — no `zone_id` in worldgen / wasm-prims / the
cold-event payloads / pixijs-render — holds; the residual `zone_id` is anchor-bookkeeping only, G's
domain.

**Why not fold F+G together to honour the literal plan?** The two were deliberately sequenced so F is
browser-verified on its own (the seam bug lived in the render origin) and G is unit-tested internals.
Merging them into one big-bang commit would lose that separation for no gain. "Less churn" is never the
reason (D-3 elsewhere proved that), but here it's about keeping the browser-verifiable change isolated.
