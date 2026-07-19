# Todo — texture-restructure

_Items move to [`completed.md`](completed.md) as they land + verify. Design: [`README`](README.md)
· decisions [`forks.md`](forks.md) · issues [`issues.md`](issues.md) · blockers [`blockers.md`](blockers.md)._

> **Status (2026-07-19).** **The migration is functionally complete.** The leaf reshape is done for
> EVERY existing texture kind (biome-thing, pawn/animal, pawn/human body-types folded to
> `<kind>.<subkind>`, `linked/`); edge readers + DSL stems flipped; **`bin/art`'s read/write core cut
> over** (R3); browser-verified (conifer/flora render, linked atlas decodes, folded human kinds serve
> 200). See [`completed.md`](completed.md). **Two items remain, both deliberately deferred** (neither has
> a current render target):

---

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

## R3-manifest · `bin/art manifest` variant/layer counters  _(non-blocking; the last R3 piece)_

`bin/art`'s read/write core is cut over (done — see completed.md). Only the **`bin/art manifest`**
generators (`_id_variant_pairs`, `_layer_count`) still walk the old `<id>.<dir>.<layer>` pose dirs
(flagged ⚠ in-code). They regenerate `content/visual/manifest/*.rd` only, which **nothing renders off**
(the edge builds its own runtime manifest) — hence non-blocking.

- [ ] Rework the two counters to the new leaf: variants = the kind's direct numeric subdirs; facing/part
      from the `<map>.<dir>.<part>.png` filename; no `<id>` level. **Truncate `variant_id ≥ 16`** out of
      the manifest (`u4`) + `log()` the drop. Do this next time `content/visual/manifest/*.rd` matters.

---

**Done when:** both deferred items land — `bin/art manifest` regenerates the new leaf, and (R2) walls/
fences/rocks fold under `biome-tile/…` once the object model places them. The functional migration
(every existing kind on `<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>`, served + rendered)
is **already done + verified**.
