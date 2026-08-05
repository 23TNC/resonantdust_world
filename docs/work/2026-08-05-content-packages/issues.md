# Issues — content packages

## I1 — the six read sites, and which two are not mechanical {#i1}

_2026-08-05, measured before planning._

| Site | Today | Change |
|---|---|---|
| [`shared/content/content.rs:16`](../../../shared/content/src/content.rs) `read_content_dir` | `read_dir`, root only, `ext == "toml"` | recurse |
| [`shared/content/content.rs:36`](../../../shared/content/src/content.rs) `content_version` | hashes the **BASENAME** | **hash the relative PATH** — see below |
| [`edge/content.rs:80`](../../../server/edge/src/content.rs) `load_disk` | root only; drops `biomes.toml` by name | recurse; drop biomes by [F2](forks.md#f2) |
| [`edge/content.rs:105`](../../../server/edge/src/content.rs) `content_keys` (R2) | **explicitly rejects nested keys** (`!rel.contains('/')`) | allow nesting |
| [`bin/content`](../../../bin/content) `_upload_root_toml` / `_download_root_toml` | `--exclude "*/*"` | drop the exclude |
| [`contentBoot.ts:37`](../../../client/webgl/src/game/definitions/contentBoot.ts) | four hard-coded `?raw` imports | `import.meta.glob` — **build-time** |

Consumers of `read_content_dir` that need no change, because they take whatever it returns:
`master/main.rs:124`, `master/defs.rs:166`, `worker/main.rs:76`, `edge/worldgen.rs:80`,
`npc/lib.rs:378`, `examples/dump_tiles.rs`.

**The fingerprint is the subtle one.** `content_version` deliberately hashes the basename so that
disk and R2 — which name the same file differently — agree. Nested, that becomes a collision:
`mods/a/things.toml` and `mods/b/things.toml` feed the hash identically, and **moving a file between
packages changes nothing**, so the hot-reload poll sees no change and never swaps. The fix is to hash
the relative path, which requires disk and R2 to produce the same relative path — they already do
(`load_disk` strips the root, `content_keys` strips the key prefix).

## I2 — `biomes.toml` is named in six places, only one of which is the filter {#i2}

_2026-08-05._ Beyond the two filters this stream changes, the string appears in:

- `edge/worldgen.rs:106` — the "content defines no biomes" error message (prose, harmless).
- `edge/worldgen.rs:259/301` — **test fixture names**, which are just source labels.
- `master/defs.rs:6` — a comment explaining where subtype ids are authored.

None of these is a behavioural dependency, so [F2](forks.md#f2)'s move away from name-matching does
not ripple. Worth recording because "grep biomes.toml" returns six hits and only two matter.

## I3 — packages will eventually want to REPLACE, and versioning only gives them SUPERSEDE {#i3}

_2026-08-05, noted, deliberately not solved._

A mod that wants a different wolf authors a new **version** of the wolf, which coexists with the
original and wins name resolution by being newest
([F6](../2026-08-04-definition-registry/forks.md#f6)). That covers "make the wolf faster" with no new
machinery, and it is genuinely the better default: the original still exists, so entities already
holding it are unaffected — the whole point of the versioning design.

What it does not cover is a mod that wants the original **gone** — a total conversion, or two mods
that disagree. Both would need real precedence rules (load order, explicit `replaces = `, conflict
detection), and every one of those is a design conversation rather than a loader flag.

Left out on purpose. The failure mode of guessing here is a precedence system nobody asked for that
later has to be unpicked; the failure mode of waiting is that someone asks for it and we build the
shape they actually need.
