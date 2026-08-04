# Issues — content is TOML-only

## I1 — the inventory: seven non-TOML paths, three readers, one of them broken {#i1}

_2026-08-04, measured._ `git ls-files content` returns twelve paths; five are the corpus
(`biomes.toml`, `materials.toml`, `needs.toml`, `things.toml`, `tiles.toml`). The other seven:

| Path | Writer | Readers (grep-proven) | Verdict |
|---|---|---|---|
| `visual/manifest/pawn.rd` | `bin/art manifest` ([bin/art:3130](../../../bin/art)) | none in code; `bin/dsl:251` uploads the folder | move ([F1](forks.md#f1)) |
| `visual/manifest/biome-tile.rd` | same | same | move |
| `visual/manifest/biome-thing.rd` | same | same | move |
| `manifest.json` | `bin/dsl reindex` ([bin/content](../../../bin/content), then named `bin/dsl` — [F7](forks.md#f7)) | `server/edge/src/content.rs:135` `load_r2` | delete ([F4](forks.md#f4)) |
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

## I8 — seven edge worldgen tests had been red since the DSL was deleted {#i8}

_2026-08-04, closed by P4._ Sweeping `.rd` out of the code turned up `cargo test` in
`server/edge`: **7 of 19 failing**, every one in `worldgen::tests`. Their fixtures build corpora in
the deleted dialect (`<tile>`, `::grass>`, `@define>`) and pass them to `Worldgen::from_sources`,
which routes to a `load()` that refuses non-TOML **by name**. Every call returns `Err`; every
`.expect("load content")` panics.

This predates this stream — it landed with [`toml-content`](../2026-08-04-toml-content/README.md)
P6 on 2026-08-04, whose completed log records a green gate. That gate was `rd`'s shared 2-pass
`cargo check` plus the shared workspace's tests; **the edge's own test binary was in neither**. A
`check` compiles a test that can never pass, and nothing in the loop ran it.

Fixed by porting the fixtures: explicit ids as [F1](../2026-08-04-toml-content/forks.md#f1)
requires, `when`/`tile`/`scatter` biome rules, and the `wg()` helper's ordinal ids kept ordinal on
purpose — those tests are *about* what reordering does to stored ids. 19/19 green.

Worth carrying forward: the repo has no single command that runs every crate's tests. That is how
this hid for a whole stream, and it is a bigger fix than this stream should make.

## I7 — a concurrent session swept P2's work into its own commits {#i7}

_2026-08-04, open (history only; the tree is correct)._ P2's changes — `bin/art`, `bin/dsl`, the
`content/visual/` deletion, and this stream's doc edits — were committed by a **different session**
working the `2026-08-04-conditions` stream in the same worktree. It ran `git add -A`, which staged
everything in the tree including mine, so P2 landed as `3d63eafe` and `0e76cc4d`, both titled
`docs(conditions): …`. My own `git commit` then found nothing to commit.

Nothing is lost and the tree is verified correct (`bin/art manifest` regenerates 7 kinds,
`def_span` resolves the conifer, `CONTENT_FOLDERS` is gone, `content/` is TOML + `servers/`). What
is wrong is the **history**: two commits claim to be about condition-card naming and actually
carry a texture-index migration.

**Not fixing it by rewriting.** The other session is still committing; a rebase or reword against
a branch someone else is actively writing to risks losing *their* work to fix the labelling of
mine. That trade is not worth it.

**Mitigation from here**: this stream stages explicit paths (`git add <path>…`) instead of
`git add -A` for the rest of its phases, so it cannot do to the other stream what was done to it.
The standing "commit freely, `git add -A` is fine" habit is only safe in a worktree with one
writer, which this is not today.

## I6 — the committed art manifest was four days stale, and that is the whole argument {#i6}

_2026-08-04, closed by P2._ The move's acceptance was "the JSON decodes to the same map as the
`.rd`". It did — for 7 kinds and every field but one. `biome-thing/default/flora` carried
`hash = 3890346443754983` in the committed `.rd`; regenerating produced `3703297076635248`.

Not a converter bug. `_kind_hash` is untouched code (sha256 over the kind's master PNGs), and
running it directly against the current tree gives the new value. The flora masters were rewritten
**2026-08-01**; the `.rd` was last committed **2026-07-30**. Nobody re-ran `bin/art manifest`, and
nothing could tell — the index was tracked, the tree it indexes is not, so git showed a clean
file describing textures that had since changed underneath it.

This is [F1](forks.md#f1)'s argument arriving as evidence rather than reasoning: a tracked index
of an untracked tree is stale by default and silent about it. In `textures/manifest/` the index
shares its subject's lifetime — regenerated when the masters are, absent when they are. Verified
as: every field except that one hash identical, and that hash equal to what the unchanged hash
function computes today.

## I5 — a fourth private copy of the content walk, still speaking `.rd` {#i5}

_2026-08-04, open until P1._ `2026-08-04-toml-content` P6 flushed out a third private content walk
(the worker's `load_speeds`). A fourth survived: `load_disk` in
[server/edge/src/content.rs:80](../../../server/edge/src/content.rs) prefers root `*.toml` but
falls back to walking `data/`, `visual/`, `material/` for `*.rd` when no TOML is found. The branch
is unreachable while the corpus exists and is pure fossil. Its unit tests still assert over
`.rd` fixture names, as do `shared/content/src/content.rs`'s version tests — harmless strings, but
they describe a dialect the repo no longer has.
