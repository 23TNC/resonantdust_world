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

## 2026-08-05 · P1 — the edge serves a tree, biomes stay server-only by DATA (3/3)

`load_disk` no longer walks anything: it **delegates to `read_content_dir`** and applies the
client-facing filter. One reader, one set of rules — this file has hosted a private copy of the
content walk before and it went stale.

**`strip_server_only` lives in the CONTENT crate**, not the edge, because what the corpus *means* is
that crate's business and the edge's business is serving what it is handed. It removes `[[biome]]`
blocks, withholds a biome-only file entirely, and — deliberately — returns a file with nothing
server-only **byte-identical**, so the common case ships the author's own formatting and comments
rather than a round-tripped copy.

The test is the point of [F2](forks.md#f2): a package writes `mods/foo/stuff.toml` carrying a
`tundra` biome beside a `snowdrift` thing. The client payload keeps `snowdrift` and does **not**
contain `tundra`. Under the old basename rule that file shipped worldgen to every client, silently.

**[I4](issues.md#i4) — a second private copy of the fingerprint, found because a test that should
have broken didn't.** `version_uses_basename_not_path` kept passing after F3, because the edge had
its own `content_version` still hashing basenames. The edge would have kept exactly the collision F3
removes: two packages' `things.toml` producing one fingerprint, so `/content-version` never moves
and no client ever refetches. Deleted, not patched — a duplicate has to be kept in lockstep through
this change and every future one, which is how the `.rd` walk went stale in this same file.

Verified: all 18 edge tests green.
