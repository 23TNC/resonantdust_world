# Todo — texture-restructure

_Items move to [`completed.md`](completed.md) as they land + verify. Design: [`README`](README.md)
· decisions [`forks.md`](forks.md) · issues [`issues.md`](issues.md) · blockers [`blockers.md`](blockers.md)._

> **Status (2026-07-19).** **The texture restructure is COMPLETE + verified.** Every existing kind on
> the new leaf `<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>`; `linked/` folded into
> `biome-tile/` (material→kind, form→variant); the `rock` stand-in reverted to a gray primitive; the
> **edge serves both canonical and named-variant stems**; **`bin/art` fully cut over** (read/write +
> `manifest`); browser-verified throughout. See [`completed.md`](completed.md). Nothing below is
> restructure work — the two follow-ons are a different *kind* of work:

---

## F1 · Art production — re-master the never-mastered kinds  _(not layout; art)_

`wall.blueprint` (a superseded 16-cell split), the raw-source `fence.*`/`rock.*`/`wall.{brick,plank}`,
and the double-encoded `wall.smooth` source masters have **no held-whole atlas** — they were never
mastered (predates this stream). Their sources are preserved under `biome-tile/default/<material>/<form>/`.

- [ ] `bin/art remaster` each to a held-whole atlas (per-kind grid config; some sources are `.psd`).
      `bin/art`'s grid-remaster write is already on the new leaf, so the output lands correctly.

## F2 · The biome-tile object model  _(a separate feature — gameplay, not textures)_

Nothing places walls/fences/rocks as biome-tile **tiles** (`kind_id ≥ 0x800`) yet, so nothing consumes
the biome-tile textures at runtime. When that feature is built:

- [ ] **P0 registry** — `textures/<type>/<subtype>/meta.json`: `form → variant_id`, tile/linked
      classification (`0x800`). Not the `kind_id` authority (the data DSL is — [F5](forks.md)).
- [ ] worldgen places biome-tile objects; the client renders them off the (already-serving) atlases.

---

**The restructure is done.** F1 (art) and F2 (object model) are follow-on work of a different kind,
tracked here only so the biome-tile textures have a clear next owner.
