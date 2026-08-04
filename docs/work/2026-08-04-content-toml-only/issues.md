# Issues — content is TOML-only

## I1 — the inventory: seven non-TOML paths, three readers, one of them broken {#i1}

_2026-08-04, measured._ `git ls-files content` returns twelve paths; five are the corpus
(`biomes.toml`, `materials.toml`, `needs.toml`, `things.toml`, `tiles.toml`). The other seven:

| Path | Writer | Readers (grep-proven) | Verdict |
|---|---|---|---|
| `visual/manifest/pawn.rd` | `bin/art manifest` ([bin/art:3130](../../../bin/art)) | none in code; `bin/dsl:251` uploads the folder | move ([F1](forks.md#f1)) |
| `visual/manifest/biome-tile.rd` | same | same | move |
| `visual/manifest/biome-thing.rd` | same | same | move |
| `manifest.json` | `bin/dsl reindex` ([bin/dsl:353](../../../bin/dsl)) | `server/edge/src/content.rs:135` `load_r2` | delete ([F4](forks.md#f4)) |
| `servers/alpha` | hand-authored | `bin/lib/common.sh:121` → `bin/lib/index.sh` | move ([F3](forks.md#f3)) |
| `servers/claude` | hand-authored | same | move |
| `servers/dev` | hand-authored | same | move |
| `servers/test` | hand-authored | same | move |

The corpus reader `read_content_dir` ([shared/content/src/content.rs:16](../../../shared/content/src/content.rs))
is **flat and TOML-only** (`read_dir`, `ext == "toml"`), so nothing under `content/visual/` or
`content/servers/` can reach a `Bundle` today — the exiles are inert, not dangerous. Worth knowing
for P2: because the walk is flat, a TOML file dropped in a `content/` **subdirectory** is also
invisible, which is why the gate in P4 checks the tracked file set rather than the loader.

## I2 — the deployed content path is broken, not just stale {#i2}

_2026-08-04, open until P1._ `content/manifest.json` lists `data/things.rd`, `data/tiles.rd`,
`visual/things.rd`, `visual/tiles.rd`, `biome/biomes.rd`. All five were deleted by
[`2026-08-04-toml-content`](../2026-08-04-toml-content/README.md) P6. `load_r2` fetches the
manifest, then fetches each key in order — so the first `r2_get_text` returns a 404 and the whole
R2 content source errors out.

Nothing noticed because every environment in `content/servers/*` runs the **Disk** source; the R2
branch is the deployed-with-credentials path and this repo has none. It is a live defect in code
that has never been exercised since the migration. [F4](forks.md#f4) removes the mechanism rather
than refreshing the file, so the defect cannot recur in the same shape.

## I3 — the R2 listing cannot be verified in this repo {#i3}

_2026-08-04, accepted limit._ `bin/keys/*` is gitignored and empty here; `bin/art`'s publish tail
says the same ("no R2 creds"). So P1's `ListObjectsV2` path compiles and unit-tests but never runs
against a bucket in this stream. Acceptance is scoped accordingly: a unit test over the key filter
(keeps `*.toml`, drops `manifest.json`, a `.rd`, a subdirectory key) plus a clean build. The first
real deploy should watch the edge's content log line; noted here so it isn't rediscovered.

## I4 — `def_span.py` resolves zero stems since the DSL died {#i4}

_2026-08-04, open until P1._ [bin/lib/def_span.py](../../../bin/lib/def_span.py) globs
`content/**/*.rd` and regexes `&thing.span set` / `&thing.texture set` / `&thing.footprint.[wh] set`
— all deleted with the dialect. `python3 bin/lib/def_span.py --all` prints its header row and
nothing else. The only `.rd` files it can still find are the art manifests, which carry no
`&thing.span`.

Consequence: `_def_span` ([bin/art:142](../../../bin/art)) swallows the failure with
`2>/dev/null || return 1`, so `art leaf-span` stops stamping `span` into a leaf's `meta.json`, and
`server/edge/src/tex_manifest.rs` reads that stamp to derive the pow2 `square` a stem packs into. A
freshly mastered leaf silently gets `span: None`. The span is authored in `things.toml` now; the
port is one item in P1, and it must keep the module's documented span≠size≠footprint distinctions.

## I5 — a fourth private copy of the content walk, still speaking `.rd` {#i5}

_2026-08-04, open until P1._ `2026-08-04-toml-content` P6 flushed out a third private content walk
(the worker's `load_speeds`). A fourth survived: `load_disk` in
[server/edge/src/content.rs:80](../../../server/edge/src/content.rs) prefers root `*.toml` but
falls back to walking `data/`, `visual/`, `material/` for `*.rd` when no TOML is found. The branch
is unreachable while the corpus exists and is pure fossil. Its unit tests still assert over
`.rd` fixture names, as do `shared/content/src/content.rs`'s version tests — harmless strings, but
they describe a dialect the repo no longer has.
