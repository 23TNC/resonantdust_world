# Completed — content is TOML-only

_Dated evidence: what landed and how it was checked. Append chronologically._

## 2026-08-04 · P3 — the deploy topology leaves `content/` (2/2)

`content/servers/{alpha,claude,dev,test}` → `deploy/servers/`, as renames git tracked as renames.
`rd_servers_manifest` now builds from `$REPO/deploy` rather than `$CONTENT_DIR`, which is the whole
mechanical change — every other consumer (`rd index seed`, `rd redeploy`'s input tracking) goes
through that one function, so nothing else needed touching. Format unchanged per
[F3](forks.md#f3): bash reads these files, and a TOML parser in bash costs more than the rule is
worth here.

The item's stated acceptance was weak and I said so rather than collecting the easy green:
`rd index show` reads the **live DB**, so it would have printed identical rows whether or not the
move worked. The real check is the seed — `rd index seed` logs
`seed index resonantdust-dev-index-0 ← deploy/servers/dev` and writes the same 4 rows
(1 server, 1 shard, 2 cold), with `show` confirming them afterwards.

Path references re-pointed in `bin/rd`'s help, `bin/lib/index.sh`'s header + usage, and each of
the four files' own headers — plus three the item hadn't listed and the grep caught:
`docs/notes/tables.md`, the index module's `intent/README.md`, and `server/edge/src/config.rs`.
`grep -rn 'content/servers'` across `bin/ docs/ server/ deploy/` is clean.

**`content/` is now exactly the five authored corpus TOMLs.** P4 makes that a gate.

## 2026-08-04 · P2 — the art manifests leave `content/` (4/4)

**The move is proven, not asserted.** Before touching the generator I decoded all three `.rd`
manifests into a canonical `{type: {subcategories, kinds: {key: fields}}}` dict, then decoded the
regenerated JSON the same way and diffed. 7 kinds, 82 variations, every field identical — except
`biome-thing/default/flora`'s content hash, which turned out to be the committed `.rd` being four
days stale rather than a converter bug ([I6](issues.md#i6), and the cleanest possible argument for
[F1](forks.md#f1)). Regeneration is byte-deterministic across runs and every file is valid JSON.

`content/visual/` is deleted. `git ls-files content` now returns the five corpus TOMLs plus
`servers/*`, and `textures/manifest/*.json` is untracked by inheritance — the gitignore already
covered `/textures/`, which is exactly the property F1 wanted.

**`bin/dsl` lost the folder concept entirely**, not just the `visual` entry: `_remote_key_for`,
`_upload_folder`, `_download_folder` and the now-orphaned `_r2_sync` / `_r2_pull` are all gone,
and `_check_relpath` went from "must start with `data/`, `visual/` or `biome/`" to "a flat
`*.toml` name" — both rejection paths checked by hand. What is left is a tool that syncs
`content/*.toml` and copies single files, which is all the corpus can be now.

`bin/art`'s stubbed publish tail no longer routes the texture index through the *content* publish
path — a category error that had it pushing `content/visual/` and reindexing `manifest.json` when
the file set changed. The reference sequence is now master→R2, regenerate, upload the index with
the masters; `bin/dsl` is not in it.

One fork logged, not built: [F7](forks.md#f7) — `bin/dsl` is now named after a deleted dialect,
and the rename to `bin/content` belongs with the docs and memory that cite it, in P4.

## 2026-08-04 · P1 — the dead die first (5/5)

**The R2 index is gone, and with it the class of bug.** `load_r2` now calls `r2_list_keys` —
SigV4 `ListObjectsV2` over `<prefix>/content/`, following continuation tokens, percent-decoding
each key (rusty-s3 asks for `encoding-type=url`) — and feeds the pure `content_keys` filter. That
filter is the thing under test: given a realistic listing (the corpus, `biomes.toml`,
`manifest.json`, a leftover `visual/tiles.rd`, a nested `visual/x.toml`, and a texture key outside
the prefix) it yields exactly `[materials, needs, things, tiles].toml`. Verified by
`content_keys_keeps_root_toml_only`, with the whole edge crate compiling against rusty-s3 0.5 —
which is as far as [I3](issues.md#i3) said this could be taken without bucket credentials.

`content/manifest.json` and `bin/dsl reindex` deleted; `cmd_upload` now just pings
`/content/refresh` where it used to rewrite the index. `grep -rn manifest.json bin/` is clean; the
three remaining hits in `content.rs` are two comments explaining why the mechanism died and one
test fixture asserting the key is filtered out — descriptions of a deleted thing, not references
to a live one.

**[I5](issues.md#i5) closed**: the `.rd` facet fallback in `load_disk` is gone, so both sources
now apply one rule — root `*.toml`, sorted, minus `biomes.toml`. The rewritten
`load_disk_reads_root_toml_sorted` proves it drops `biomes.toml`, `manifest.json` **and** a nested
`visual/nested.toml`, which matters because the R2 side must agree and `content_keys` drops nested
keys too. All 5 edge content tests green; the shared workspace's 80 green including the golden
fixture after the same fixture rename there.

**[I4](issues.md#i4) closed, with the failure mode designed out.** `def_span.py` reads
`content/things.toml` through a small table-aware scanner (Python 3.10 on this host has no
`tomllib`, and `bin/` must work on a bare checkout — so it tracks which TOML table it is inside
and reads three keys out of `[[thing.part]]`, deliberately not a parser). `biome-thing/default/
conifer` → `2`, and `--all` lists all five texture-bearing defs. The part that matters: an empty
read is now a **loud exit 2**, distinct from exit 1's legitimate "no span authored, fall back",
and `bin/art`'s `_span_side` — which swallowed stderr with `2>/dev/null` and is exactly where the
silence lived — now surfaces it. End to end: `_span_side biome-thing/default/conifer` → `256`,
the documented span-2 square, restored.

Also fixed en route: `content.rs`'s module header and `ContentSource` docs described a "DSL
corpus" read as `data`/`visual`/`material` facets, none of which has been true since P6.

One fork logged, not built: [F6](forks.md#f6) — the `/content` payload's `"rd"` key is a live
three-consumer wire contract, so it moves to P4 as its own commit rather than riding a deletion.

## 2026-08-04 · P0 — the inventory, frozen (1/1)

`git ls-files content` measured, not assumed: 12 tracked paths, 5 of them the corpus. Every one
of the other 7 traced to its writer and its readers by grep, recorded in
[`issues.md` I1](issues.md#i1) with a verdict per path.

Three things the count turned up that nobody was looking for:

- **[I2](issues.md#i2)** — `content/manifest.json` is not just misplaced, it is stale AND
  load-bearing: it names five `.rd` keys deleted by the previous stream's P6, so the edge's R2
  content source 404s on its first fetch. Unnoticed because every configured environment runs the
  Disk source.
- **[I4](issues.md#i4)** — `bin/lib/def_span.py` still greps the deleted `.rd` dialect;
  `--all` now resolves **zero** stems, so `art leaf-span` silently stops stamping the span that
  `tex_manifest` uses to size a stem's pow2 packing square.
- **[I5](issues.md#i5)** — a *fourth* private copy of the content walk survived the previous
  stream's cull, in the edge's `load_disk`, still speaking `.rd`.

Also established for the plan: `read_content_dir` is flat and TOML-only, so nothing under
`content/visual/` or `content/servers/` can reach a `Bundle` — the exiles are inert. That is why
P4's gate checks the tracked file set rather than trusting the loader to reject a stray.
