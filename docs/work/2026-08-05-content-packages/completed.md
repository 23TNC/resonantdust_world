# Completed — content packages

_Dated evidence: what landed and how it was checked. Append chronologically._

_Nothing yet._

## 2026-08-05 · P0 — the reader recurses, the fingerprint stops colliding (3/3)

`read_content_dir` walks the tree and names each source by its path relative to the content root.
Two details that are not incidental:

- **Sorted by NAME, not by traversal order.** `read_dir` order is filesystem-defined, so two
  machines walking one tree would otherwise produce the corpus in different orders — and biome
  evaluation priority depends on order.
- **Separators normalized to `/`.** A source's name is hashed into the fingerprint AND shipped to
  clients, so it must be the same string on every platform.

**The fingerprint now keys on the path** ([F3](forks.md#f3)). The old test asserted that a
directory-prefix change did NOT move the version — that assertion was the bug, stated as a
property: `mods/a/things.toml` and `mods/b/things.toml` would have hashed identically, and moving a
file between packages would have changed nothing at all, so the hot-reload poll would never fire.
Both cases now assert the opposite.

**The real corpus is untouched**: the golden fixture passes with no re-bless, because every current
file sits at the root and its relative name equals its basename. That is the migration proof — the
tree walk is a superset of the flat read, not a replacement for it.
