# Design — `shared/codec` (the shape)

**Authoritative reference model:** [`reference-model.md`](reference-model.md) — the
`definition` / `position` / `data` split (2026-07-14). `object-model.md` + `references/` below
describe the current **code** (v1), superseded by it pending a re-cut.

_Last updated: 2026-07-14._

- **[`object-model.md`](object-model.md)** — the object taxonomy (type/kind/variant), the packed
  `*_reference` u64 layouts, and the cold/hot shard classes. Contains the still-**open** decisions.
- **[`references/`](references/)** — the bit-layout specs:
  [`object-reference.md`](references/object-reference.md),
  [`hot-cold-references.md`](references/hot-cold-references.md),
  [`spatial-references.md`](references/spatial-references.md),
  [`reference-vs-id.md`](references/reference-vs-id.md).
