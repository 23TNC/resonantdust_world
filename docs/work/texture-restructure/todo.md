# Todo — texture-restructure

_Items move to [`completed.md`](completed.md) as they land + verify. Design: [`README`](README.md)
· decisions [`forks.md`](forks.md) · issues [`issues.md`](issues.md) · blockers [`blockers.md`](blockers.md)._

> **Status (2026-07-19).** **The leaf reshape is DONE + verified for every existing texture kind with
> a render target** (see [`completed.md`](completed.md)): P1 texpath+marigold, migrate (biome-thing +
> pawn/wolf + `linked/`), edge readers, DSL stems, browser render (conifer/flora) + linked-atlas decode.
> What's below is **phase 2** — none of it has a current render target, so it's ordered by value,
> not urgency.

---

## R1 · Human-pawn body-type subkinds  _(the last un-reshaped existing art)_

`pawn/human/{dead,male,female}/{average,thin,fat,fit}` still use `<subkind>` for **body type** —
`migrate_leaf.py` currently **skips** them (a blind subkind-drop would collide average/thin).

- [ ] **Decide the modeling** (a fork): the def model is `type/subtype/kind/variant` with **no
      `body-type` field**. Does body-type become the `variant` (art-variations flattened under it), a
      new `kind` per body-type, or something else? Ties into how humans get spawned. — *the one genuine
      user-input piece here.*
- [ ] Extend `migrate_leaf.py` for the chosen mapping; reshape the human trees; drop the old leaves.

## R2 · The full biome-tile fold  _(rename `linked/` → `biome-tile/`; deferred — object-model-coupled)_

The linked **leaf reshape** is done (`linked/<kind>/<variant>/<map>.<dir>.<part>` + `atlas.json`).
The **fold** — renaming to `biome-tile/<biome>/<material>/<form>/` (material→kind, form→variant,
`kind_id ≥ 0x800`) — is **not**, and is premature: walls/fences/rocks aren't worldgen-placed, so nothing
renders differently. Do this **when the biome-tile object model / wall placement lands** ([F3](forks.md)).

- [ ] **P0 registry** — `textures/<type>/<subtype>/meta.json`: `form → variant_id` (F2), tile/linked
      classification (`0x800`), sheet-split info. **Not** the `kind_id` authority (that's the data DSL —
      [F5](forks.md)). Author for `biome-tile/default` + the linked materials.
- [ ] `texpath` biome-tile fold (given a linked source, emit `biome-tile/<biome>/<material>/<form>/…`);
      edge + DSL + client resolve the **named-variant** biome-tile linked stems.
- [ ] **Re-master** the kinds not yet held-whole atlases: `wall.blueprint` (still 16-split), the empty
      `fence.*`/`rock.*`, and the double-encoded `wall.smooth` `1.l.0.l.0` source masters.

## R3 · `bin/art` regeneration into the new leaf  _(offline tooling; no pending new art)_

So *newly generated* art is written in-shape (this stream only reshaped **existing** art). Large bash
surface in `bin/art` (~3127 lines) + `bin/lib/*.py`.

- [ ] Every **write** site emits the new leaf: `split_layers.py`, `generate.py`, `emissive.py`,
      `marigold/delight.py` (all texpath consumers already reshaped), `meta.py`.
- [ ] The **manifest walk** (`_kind_maps`, variant/part counters) globs the new leaf; **truncate
      `variant_id ≥ 16`** out of the manifest (`u4`) — and `log()` the drop (no silent cap).

---

**Done when:** `textures/` is `<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>` end to end —
including walls/fences/rocks under `biome-tile/…` and human body-types resolved — the registry
authoritative, `bin/art` writing the new leaf, browser-verified, old tree dropped.
