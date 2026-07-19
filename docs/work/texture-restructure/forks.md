# Forks — texture-restructure

_Execution decisions. The **shape** is Decided upstream in
[`texture-layout/`](../../components/dev/textures/design/texture-layout/README.md) — not re-litigated
here. These are the calls that come up **carrying it out**._

---

## F1 · Variant folder = named OR numeric (decided, user) — 2026-07-19

**Decision.** `<variant>` accepts **both**: numeric (`0..15`, art variations straight off a sprite
sheet — flora etc. have many) and **named** (a linked object's **form**: `wall`/`fence`/`rock`). Both
resolve to `variant_id`; the `type/subtype` registry ([todo P0](todo.md)) maps names → index. **≥16
truncates** out of the manifest (`u4`) — extra on-disk variations exist but go unused, and the manifest
walk must `log()` the drop, not silently cap.

## F2 · `form → variant_id` numbering (open — proposal) — 2026-07-19

**Context.** Linked forms need a stable `variant_id`. Numbering lives in the `type/subtype` registry,
not hardcoded in `texpath.py`.

**Proposal (confirm).** Assign in the registry by first-appearance / explicit list, e.g.
`wall=0, fence=1, rock=2, …`, reserving low indices for the common forms. Keep it **explicit in the
registry file** (not derived from a `readdir` order, which is unstable). Open until the registry is
authored (P0).

## F3 · Linked old-variant remapping (open — needs inspection) — 2026-07-19

**Context.** Old linked leaves like `wall.smooth/1.l.0/0..8/` carry numeric `<variant>` folders. In the
new model a linked object's `<variant>` is the **form**, so these old numerics must map to *something* —
but it's unclear whether they were **auto-tile connectivity pieces** (→ belong in `dir`/`part`, the `l`
facing's sub-indices) or genuine **art variations** (→ stay variations, but of which form?).

**To clear.** `find`/inspect the actual `linked/*/1.l.0/*` contents before P3 collapses them; the mapping
follows what they are. Do **not** blind-collapse. Flag per-kind if they differ.

## F4 · Dense-storage width is a SEPARATE stream (boundary) — 2026-07-19

**Decision.** This stream is **file structure only**. Folding `linked/`→`biome-tile` in the *texture
tree* does not require the runtime dense-storage change. The consequence — a biome-tile dense cell must
hold `kind_id`(u12)+`variant_id`(u4) = **u16**, so the cold-tile `Vec<u8>` widens `u8→u16` in
codec/worldgen/edge — is **its own server-side work** (touches the [cold-rework](../cold-rework/README.md)
storage), triggered by the same decision. Kept out of here so the texture migration isn't gated on a
server refactor. Confirm the split with the user ([blockers B1](blockers.md#b1)).
