# Plan — content packages

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), measurements in [`issues.md`](issues.md)._

## P0 — the reader recurses, and the fingerprint stops colliding

- [x] Make `read_content_dir` walk `content/**/*.toml`, yielding the RELATIVE path as each source's
      name, sorted. Acceptance: a crate test over a `a.toml` + `mods/foo/b.toml` fixture returns
      both, named `a.toml` and `mods/foo/b.toml`, in that order. → `read_tree`; sorted by NAME not
      traversal order (`read_dir` order is filesystem-defined), separators normalized to `/`.
- [x] Hash the relative PATH in `content_version`, not the basename ([F3](forks.md#f3)).
      Acceptance: a test shows `mods/a/x.toml` and `mods/b/x.toml` with identical text now
      fingerprint DIFFERENTLY, and that moving a file changes the version. → both asserted; the
      old "a dir-prefix change doesn't move it" case was inverted, since that IS the collision.
- [x] Prove the real corpus is unchanged by the walk. Acceptance: the golden fixture passes with no
      re-bless — every current file is at the root, so its relative name equals its basename. →
      passes untouched; 23 content tests + 63 codec green.

## P1 — the edge serves a tree, and biomes stay server-only by DATA

- [x] Recurse in the edge's `load_disk`, keeping relative paths as source names. Acceptance: a unit
      test with a nested `mods/foo/things.toml` sees it served; the existing root files keep their
      names. → `load_disk` now DELEGATES to `read_content_dir` rather than keeping its own walk;
      found and deleted a second private `content_version` while there ([I4](issues.md#i4)).
- [x] Strip `[[biome]]` blocks from every served source rather than dropping `biomes.toml` by name
      ([F2](forks.md#f2)). Acceptance: a test where biomes live in `mods/foo/anything.toml` shows
      the client payload carries that file's non-biome defs and NO biome rules. →
      `strip_server_only` in the CONTENT crate (corpus meaning is its business); a file with nothing
      server-only comes back byte-identical, so the common case keeps the author's formatting.
- [x] Allow nested keys in the R2 `content_keys` filter. Acceptance: its test keeps
      `mods/foo/things.toml`, still drops `manifest.json` and a `.rd`, and still yields sorted
      relative names. → and it now keeps `biomes.toml` as a KEY, because the server-only filter
      runs on the fetched DATA — a key tells you nothing about what is inside it.

## P2 — the publisher and the client embed stop enumerating

- [ ] Drop `--exclude "*/*"` from `bin/content`'s upload and download so a package tree round-trips.
      Acceptance: `bin/content --help` describes a tree; the sync commands carry no root-only flag.
- [ ] Replace `contentBoot.ts`'s four `?raw` imports with `import.meta.glob("@content/**/*.toml")`.
      Acceptance: `npx tsc --noEmit` clean and the client boots offline from the embed with the
      same def count it has today.

## P3 — a package proves it, then the docs say so

- [ ] Add a throwaway `content/mods/example/` package defining one new thing, load the stack, and
      confirm it registers. Acceptance: the edge logs a higher `definitions=` count and the client
      resolves the new def by name; the package is DELETED before the phase closes.
- [ ] Write the package model into `VARIABLES.md` § TOML content schema: a folder is a package, no
      manifest, collisions are load errors, biomes are stripped by data. Acceptance: docs-check
      green; `rd content-check` still passes on the nested tree.
