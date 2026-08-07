# logs-drop — a felled tree leaves logs where it stood

**What** (user, 2026-08-07): "implement the log things drop when trees are felled" — the
successor the lumberjack stream reserved by name (its F5/I9: "place a thing — logs —
where the tree stood; `yields` lands beside `destroy`, additive, no reshape").

## Design stance

- **`yields` is authored on the CARRIER BINDING, not the interaction**
  ([F1](forks.md#f1)): `interactions = [{ name = "cut_down", yields = "logs" }]` on the
  TREE only. The interaction is shared by shrub and cactus (lumberjack F6) — an
  interaction-level yield would make felled shrubs drop logs. The binding already
  carries per-carrier data (`magnitude` — the water binds `drink 3`); `yields` is the
  same pattern.
- **Yield = the destroy SET carries the yielded kind instead of 0**
  ([F2](forks.md#f2)): the lumberjack composer already addresses the carrier's
  `(cold_row, cell)` in one place; with a yield the ONE clearing
  `PROMOTE SET … TYPE_BIOME_THING <cell> 0 0` becomes
  `… <cell> <logs kind_reference> 0` — a REPLACE, not a clear-then-place pair racing
  over one cell. "Place a thing where the tree was" is literally one operand.
- **Logs are ordinary scatter** ([F3](forks.md#f3)): a new `[[thing]] logs` on the thing
  layer, PLACEHOLDER visual (white texture + log-brown tint, the shrub/rock pattern) —
  no new art pipeline work this stream. Logs offer NO interactions yet (no cut_down —
  they are not tree-like; hauling/inventory is the recorded successor, not this stream).
- **Everything downstream is proven machinery**: a kind-carrying thing override draws
  through the same client path the tombstone rides (lumberjack's edge fixes); the menu
  map's override branch records the new kind; reload-persistence comes for free.

## What exists (audited 2026-08-07, post-lumberjack)

- The worker's destroy arm holds `carrier = (position, is_thing, cold_row)` and emits
  the clearing SET — the yield is one branch in that composer.
- `thing_interactions(object_id) -> Vec<(name, magnitude)>` — the binding accessor the
  yield must thread through (a stride change or a params struct — the loader's call).
- Registry numbering is append-only; a NEW thing def appends a kind id. The master
  seeds the registry at boot from the corpus — lumberjack I11: after ANY shared/content
  change, rebuild master + orchestrator + worker + npc or defs silently fail to
  resolve.
- Golden guards the thing tables + bindings; re-bless per I5 discipline.
