# Content packages — the corpus is a TREE, and a folder is a mod — 2026-08-05

_Components: `shared/content`, `server/edge` (both content sources), `server/master`,
`server/worker`, `client/npc`, `client/webgl` (the offline embed), `bin/content`, `bin/rd`. Plan in
[`todo.md`](todo.md); decisions in [`forks.md`](forks.md); measurements in
[`issues.md`](issues.md)._

## The user's call

> "Let's update our content folder to recursively include toml in that folder instead of relying on
> hard coded files. In this way we should be able to create packages that effectively function as
> mods." — 2026-08-05

## What "recursive" actually costs

The corpus is read in **six** places and every one assumes a flat root
([I1](issues.md#i1)). Two of them are not simply "add a `walkdir`":

- **`content_version` hashes the BASENAME**, not the path
  ([`content.rs:36`](../../../shared/content/src/content.rs)). That was correct while the corpus was
  flat and is a *collision* the moment it nests: `mods/a/things.toml` and `mods/b/things.toml`
  hash the same, and **moving a file between packages would not change the fingerprint at all** —
  so the hot-reload poll would not notice. It must hash the relative path.
- **The client's offline embed hard-codes four `?raw` imports**
  ([`contentBoot.ts:37`](../../../client/webgl/src/game/definitions/contentBoot.ts)). A tree cannot
  be enumerated by hand; it needs `import.meta.glob`, which is a build-time change rather than a
  runtime one.

The rest are mechanical: the edge's `load_disk`, the edge's R2 `content_keys` (which *explicitly*
drops nested keys today), and `bin/content`'s `--exclude "*/*"`.

## The design stance

**A package is a folder, and that is the whole mechanism.** No manifest, no registration, no load
order file — drop `content/mods/foo/` in and its defs are part of the corpus. This is only possible
because the previous streams removed every reason the corpus needed structure:

- ids come from the **registry**, not from corpus position
  ([definition-registry F1](../2026-08-04-definition-registry/forks.md#f1)), so a package's files
  can load in any order without renumbering anything;
- the taxonomy is **authored**, so a def's identity does not depend on which file it sits in;
- `(taxonomy, version)` uniqueness is already enforced
  ([F17](../2026-08-04-definition-registry/forks.md#f17)), so two packages colliding is a **load
  error by construction** rather than a silent last-writer-wins.

That last point is why this is safe to do now and would not have been a year of design ago.

## The one real decision

**How does server-only content stay server-only when a mod can name its file anything?**

Today the edge withholds biomes from clients with `n != "biomes.toml"` — a basename check that a
package trivially defeats (`mods/foo/my-biomes.toml` would ship worldgen rules to every client).
Resolved in [F2](forks.md#f2) by filtering on **what the data is** rather than what the file is
called; the reasoning and the rejected alternatives are there.

## Not in scope

**Override semantics.** A mod that wants to change an existing def does it the way the game already
does — by authoring a **new version** of that def
([F6](../2026-08-04-definition-registry/forks.md#f6)), which coexists with the original and wins
resolution by being newest. There is no separate override mechanism and this stream does not build
one; if packages later need to *replace* rather than *supersede*, that is a design conversation, not
a loader flag ([I3](issues.md#i3)).
