# Design — `shared/codec` (the shape)

**Bit layouts:** [`docs/VARIABLES.md`](../../../../VARIABLES.md) is authoritative for every
cross-component variable's name, width, and layout (2026-07-15). Go there first for *what a
variable is*; the docs here explain *why*.

**Reference model:** [`reference-model.md`](reference-model.md) — the `definition` / `position` /
`data` split, authoritative for the reasoning behind those shapes. The re-cut it describes is
**done**: `object.rs`/`refs.rs` match it (2026-07-14, browser-verified).

`object-model.md` + `references/` below are the **superseded v1** (a `u64 object_reference` packing
type+kind+position+data). They no longer match the code and are kept as historical rationale.

_Last updated: 2026-07-15._

- **[`object-model.md`](object-model.md)** — the object taxonomy (type/kind/variant), the v1 packed
  `*_reference` u64 layouts, and the cold/hot shard classes. Contains still-**open** decisions.
- **[`references/`](references/)** — the v1 bit-layout specs, each carrying a superseded banner:
  [`object-reference.md`](references/object-reference.md),
  [`hot-cold-references.md`](references/hot-cold-references.md),
  [`spatial-references.md`](references/spatial-references.md),
  [`reference-vs-id.md`](references/reference-vs-id.md).
