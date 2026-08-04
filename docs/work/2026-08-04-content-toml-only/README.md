# content is TOML-only — the generated and the deployed leave the corpus — 2026-08-04

_Components: `content/`, `bin/` (`art`, `dsl`, `rd`, `lib/`), `server/edge` (the `/content`
source), `shared/content`. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
measured inventory in [`issues.md`](issues.md)._

## The user's call

> "Please create a folder of work to move non-toml out of content." — 2026-08-04

Follows [`2026-08-04-toml-content`](../2026-08-04-toml-content/README.md), which retired the DSL
and made `content/*.toml` the corpus. That stream deliberately left one live exception and, as
this stream's inventory shows, three more files nobody had counted.

## What is actually still in there

`git ls-files content` returns twelve paths. Five are the corpus. The other seven are not:

| Path | What it is | Who writes it | Who reads it |
|---|---|---|---|
| `visual/manifest/{pawn,biome-tile,biome-thing}.rd` | generated ART existence index of the texture master tree | `bin/art manifest` ([bin/art:3116](../../../bin/art)) | **nobody** — `bin/dsl` uploads it to R2 and no code parses it |
| `manifest.json` | the R2 corpus key index | `bin/dsl reindex` | `server/edge` `load_r2` — and it lists **five `.rd` keys that no longer exist** |
| `servers/{alpha,claude,dev,test}` | index routing topology per environment | hand-authored | `bin/lib/index.sh` via `rd_servers_manifest` |

None of these is game content. Two of the three groups are *generated*, and the third is
*deployment configuration*. They sit in `content/` by history, not by design.

## The design stance

**`content/` is authored, TOML, and the game's.** One rule, mechanically enforced ([P4](todo.md)):
a tracked file under `content/` is a `*.toml` file a human wrote to describe the world. Anything a
tool emits, and anything describing where servers live, is not that.

The corollary is where the exiles go. A **generated index belongs beside the tree it indexes**
([F1](forks.md#f1)) — `textures/manifest/`, untracked exactly as `textures/` is, because a tracked
index of an untracked tree is stale in every fresh checkout and nothing can prove otherwise. A
**machine-written, machine-read index is JSON** ([F2](forks.md#f2)); TOML earns its place by being
hand-edited, and nobody hand-edits an existence index. **Deployment topology is deploy's**
([F3](forks.md#f3)) — `deploy/servers/<env>`, format unchanged, because bash reads it and a TOML
parser in bash is a worse thing than a column file in the right folder.

## The bug this exposes

`content/manifest.json` is not merely misplaced — it is **stale and load-bearing**. The edge's R2
content source fetches it and then fetches each key it names, and every key it names is a `.rd`
file deleted on 2026-08-04. The deployed content path is broken right now; only the dev Disk
source keeps the stack up. The fix is not a fresher index: it is **no index** ([F4](forks.md#f4))
— the edge lists `<prefix>/content/*.toml` from the bucket, and the whole class of
stale-second-place-that-records-which-files-exist stops being possible.

Two more readers of the dead dialect fall out with it: `bin/lib/def_span.py`, which greps the
deleted `.rd` corpus for `&thing.span` and today resolves **zero** stems (so freshly mastered
texture leaves lose the span stamp that sizes their pow2 packing square), and a **fourth private
copy** of the content walk still sitting in the edge's `load_disk` as a `.rd` facet fallback.

## Where this ends up

The destination is **one texture index, not two** ([F5](forks.md#f5)). The edge's
`/textures-manifest` already scans the same master tree and already serves hash, maps, grid, span
and square to the client; the art manifest exists because it additionally carries variant, layer,
part and subkind *existence* — the data the future `^r2` resolver needs and `tex_manifest` lacks.
Folding those four in makes `/textures-manifest` the single index and retires the standalone
generator. That is [P5](todo.md), it is genuinely bigger than the move, and it may spin out into
its own stream — but it is written down here so the move is made *toward* it and not across it.
