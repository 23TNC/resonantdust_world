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

## 2026-08-05 · P2 — the publisher and the client embed stop enumerating (2/2)

`bin/content` syncs the tree: the `--exclude "*/*"` is gone from both directions, so a package
round-trips as a package. `_check_relpath` went from "a flat file name" to "a path at any depth" —
`mods/foo/things.toml` validates, `../escape.toml` is still refused, since an upload arg becomes a
bucket key and must stay inside the corpus.

**The client embed globs.** Four named `?raw` imports could not see `content/mods/foo/` — and a
package that only worked once you were online would be a trap, since the embed is precisely the
offline path. `import.meta.glob("@content/**/*.toml", { eager: true })`, eager because this is the
no-network path and must not depend on the network.

Two details worth stating:

- Names are made **relative to the content root and sorted**, so the embed presents the corpus
  exactly as `/content` does — same names, same order.
- Biome definitions **do** ride along in the embed, where the server strips them
  ([F2](forks.md#f2)). Harmless — the loader ignores what it has no use for — and the alternative is
  shipping a TOML parser in the bundle to remove them.

Verified: `npx tsc --noEmit` clean, and the client boots from the embed with the **same 6 tiles and
11 things** it had before.

## 2026-08-05 · P3 — a package proves it, and the docs say so (2/2)

**The drill.** A throwaway `content/mods/example/` defining a `moonpetal` thing, plus its own
`biomes.toml` — deliberately named the thing a basename check *would* have caught, to show the
filter no longer depends on the name:

| | before | after |
|---|---|---|
| registry rows | 182 | **183** |
| edge `definitions=` | 17 | **18** |
| `kind_ids_used` | 11 | **12** |

In the browser: `mods/example/things.toml` appears in the served payload, `moonpetal` resolves to
`0x20000010`, and **no `[[biome]]` block reached the client at all** — neither the root file nor the
package's own. Package deleted afterwards; golden green again.

**[I5](issues.md#i5) — the third private content reader, and the worst one.** Adding the package
should have failed the golden instantly. It passed, because `golden.rs`'s `corpus()` had its own
flat `read_dir` and had simply gone blind to packages. It would have kept passing while a mod added
definitions it never checked. An oracle that reads the corpus differently from the code is not an
oracle. Fixed, and it failed correctly on the next run — `moonpetal` where it expected `tree`.

That is three private copies across two streams (the edge's `.rd` facet walk, the edge's
`content_version`, this), each invisible until something that should have broken didn't. The rule
worth carrying: **a change to how content is read must grep every reader, tests included.**

**[I6](issues.md#i6)**: a package sorts before the base corpus and shifts a *fresh* world's seed
numbering. Benign — positional numbering is only the seed, and `ensure_definition` refuses to
renumber a running world — but it looks alarming in a diff, so it is written down.

`VARIABLES.md` now states the model: the corpus is a tree, a folder is a package, no manifest,
collisions are load errors, server-only content is filtered by data, and a package changes an
existing def by authoring a new **version** rather than overriding it. `rd content-check` verified
against a genuinely staged nested file rather than assumed.
