# Todo — texture-restructure

_Items move to [`completed.md`](completed.md) as they land + verify. Design: [`README`](README.md)
· decisions [`forks.md`](forks.md) · issues [`issues.md`](issues.md) · blockers [`blockers.md`](blockers.md)._

> **Status (2026-07-19).** **The migration is functionally complete.** The leaf reshape is done for
> EVERY existing texture kind; edge readers + DSL stems flipped; **`bin/art`'s read/write core cut over**
> (R3); **`linked/` folded into `biome-tile/`** (R2) + the `rock` stand-in reverted to a gray primitive;
> browser-verified throughout. See [`completed.md`](completed.md). What remains is tied to the **biome-tile
> object model** (placing walls/fences/rocks as tiles) — a separate feature, not this stream — plus a
> non-blocking `bin/art manifest` rework.

---

## R2-serving · biome-tile named-variant linked stems + re-master  _(gated on wall placement)_

The **texture fold is done** (`biome-tile/default/<material>/<form>/`; `wall.smooth` a clean held-whole
atlas — see [`completed.md`](completed.md)). What's left only matters once the object model actually
**places** walls/fences/rocks as biome-tile tiles (`kind_id ≥ 0x800`) — there is no consumer today
(the `rock` thing was reverted to a primitive):

- [ ] **P0 registry** — `textures/<type>/<subtype>/meta.json`: `form → variant_id` (F2), tile/linked
      classification (`0x800`). **Not** the `kind_id` authority (that's the data DSL — [F5](forks.md)).
- [ ] Edge/DSL/client resolve the **named-variant** biome-tile linked stems (variant = the form, e.g.
      `wall`) — `register_kind`/`master_map_rel` currently assume canonical variant `1`.
- [ ] **Re-master** the un-mastered kinds to held-whole atlases: `wall.blueprint` (superseded 16-split),
      the raw-source `fence.*`/`rock.*`/`wall.{brick,plank}`, and the double-encoded `wall.smooth` source
      masters. All preserved under `biome-tile/default/…` awaiting `bin/art` re-master.

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
