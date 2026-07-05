# Texture paths & the folder-per-variant migration

The canonical description of how texture files are named and laid out on disk, in
R2, and over HTTP — and the phased plan for migrating from the current flat-file
layout to a **folder-per-variant** layout.

Read this before touching `bin/art`, `bin/lib/*.py`, `marigold/*.py`, the
`server/src/tex_*` / `textures.rs` resolvers, or `pixijs/src/textures/*`.

> Status: **Phases 1–4 landed** (2026-07-04) — pipeline write side, the on-disk
> migration (non-destructive copy), the `art manifest` walk, and the server readers
> all use the new group-less layout. Remaining: the **R2 step** (upload key + prune +
> gc), **Phase 5** (client multi-map fetch, deferred), a re-master of `wall.smooth`
> (pre-broken), and dropping the old `<group>` trees once verified. The server is
> edited but UNCOMPILED here (no cargo in this env) — build via the project toolchain.
> When the old trees are dropped this doc supersedes the `asset-path-vocabulary` note.

---

## Two orthogonal axes — don't conflate them

The word **"master"** appears on both axes. Keep them separate or this whole
scheme reads as contradictory:

1. **Lifecycle map** — *what a file is in the art pipeline*: `template` (pose
   reference) → `sprite` (raw gen art) → `diffuse` (mastered) → `albedo` /
   `normal` / `emissive` / `packed` / … (derived). Today this is spread across
   three sibling **source trees** `textures/{master,sprites,templates}/`. **This
   axis is what we collapse:** the lifecycle becomes the *filename* inside one
   folder, and the three source trees merge into one.

2. **Serving tier** — *at what resolution / derivation the client gets bytes*:
   `master` (full-res source) → `preview` (half-res) → `lod/<size>` → `geo`.
   These are HTTP route prefixes and derived-cache subdirs. **This axis is
   unchanged.** `/textures/master/<stem>`, `/textures/lod/<hash>/<size>/<stem>`
   stay exactly as they are.

So "drop the group" = collapse **axis 1's** source trees. It has nothing to do
with the `master`/`lod`/`geo` serving tiers of **axis 2**.

---

## The new canonical layout

```
textures/<category>.<subcategory>/<kind>.<subkind>/<id>.<dir>.<layer>/<variant>/<map>.<ext>
        └────── source root ──────┘└─ pose identity ─┘└ variant ┘└ map file ┘
```

- `<category>.<subcategory>` — e.g. `pawns.human`, `world` (base subcategory `.0`
  is implicit, so `world` = `world.0`). Directory segment, dots kept.
- `<kind>.<subkind>` — e.g. `male.fit`, `wall.smooth`, `conifer`. Directory
  segment, dots kept. Named subkinds are their own dirs.
- `<id>.<dir>.<layer>` — the **pose identity** folder. `id` (usually `1`), `dir`
  ∈ `{n,e,s,l}` (west is mirrored east, never on disk; `l` = linked/autotile),
  `layer` (integer). All variants of one pose live under here.
- `<variant>/` — one variant = one **co-located map set**. `variant` is an
  integer (a sequential index, or a generation seed — see below).
- `<map>.<ext>` — the file itself: `albedo.png`, `normal.png`, `diffuse.png`,
  `emissive.png`, `packed.png`, `packed_residual.png`, `albedo_residual.png`,
  `albedo_shading.png`, `sprite.png`, `template.png`, `prompt.txt`, …

The `<group>` segment (`master`/`sprites`/`templates`) is **gone** — the map
filename already distinguishes lifecycle stage, and every map name is distinct,
so there are no collisions when they share a folder.

### What lives where — worked examples

| Asset | Old (flat) | New (foldered) |
|---|---|---|
| tree albedo | `master/world/conifer/1.s.0.1.albedo.png` | `world/conifer/1.s.0/1/albedo.png` |
| tree normal | `master/world/conifer/1.s.0.1.normal.png` | `world/conifer/1.s.0/1/normal.png` |
| wall (linked) | `master/linked/wall.smooth/1.l.0.0.albedo.png` | `linked/wall.smooth/1.l.0/0/albedo.png` |
| linked base (sparse) | `master/linked/rock.default/1.diffuse.png` | `linked/rock.default/1.s.0/1/diffuse.png` |
| wolf variant | `master/pawns.animal/wolf/1.n.0.124.albedo.png` | `pawns.animal/wolf/1.n.0/124/albedo.png` |
| human master | `master/pawns.human/male.fit/1.e.0.0.diffuse.png` | `pawns.human/male.fit/1.e.0/0/diffuse.png` |

The leaf folder is the whole co-located set — e.g.
`pawns.animal/wolf/1.n.0/124/` holds `sprite.png`, `diffuse.png`, `albedo.png`,
`normal.png`, `emissive.png`, `albedo_residual.png`, `packed.png`,
`packed_residual.png` — everything that used to be `1.n.0.124.*.png`.

### Loose files that do NOT fold into a leaf

Some inputs are not per-variant and stay at the `<category>.<subcategory>` level
as loose files (exactly where they are today, minus the `sprites/`/`templates/`
group prefix):

- **Sprite sheets** — a multi-blob sheet like `pawns.human/male.e.0.0.sprite.png`
  packs fit/fat/average × N into one image; `art remaster` splits it into the
  per-variant `male.<subkind>/…/<variant>/diffuse.png` leaves. The sheet itself
  has no single variant, so it stays loose at `pawns.human/`.
- **Alias sidecars** — `pawns.human/male.alias` (level-1 metadata that drives the
  sheet split). Loose at the category level.
- **PSD authoring sources** — `linked/rock.default/Rock_Atlas.psd`. Loose in the
  kind dir.
- **Generation prompts** — `<from>/<seed>.prompt.txt` (art generate). Keyed by seed
  and shared across the e/s/n leaves it generates, so it's a loose seed-keyed
  sidecar at the `<from>` level, not a per-variant leaf member.

Per-variant sprites (e.g. wolf, where each blob already has its own seed) *do*
fold into the leaf, because the remaster preserves the seed as the variant, so
`sprite.png` and `albedo.png` share the same `<variant>/` folder.

---

## Normalization on write (defaults)

Not every asset names all four `id.dir.layer.variant` fields today — linked kinds
use `1.diffuse.png` (id only) or `1.l.0.diffuse.png` (id.dir.layer, no variant).
**We normalize on write:** the pipeline always emits the fully-qualified
`<id>.<dir>.<layer>/<variant>/` form, filling missing fields with the canonical
defaults so disk is never sparse and no reader has to guess:

| Field | Default | Note |
|---|---|---|
| `dir` | `s` | south, the single-facing default (`l` for linked) |
| `layer` | `0` | |
| `variant` | `1` | matches the server's current `1.s.0.1` canonical instance |

These are the same defaults the `bin/art` arg parser (`_parse_sheet_ref`) and the
server (`master_albedo_rel`) already apply when reading a sparse reference — we
just now *materialize* them into the directory names.

Read-side defaulting survives in exactly one place: a **stem** the DSL/client
requests may still omit the facing (`world/conifer` → server picks `s`). That's a
URL-contract convenience, not a disk shape.

---

## Migration phases

Two orderings matter and they differ:

- **Development order** — Phases 1, 3, 4 are independent (nothing reads the new
  layout until files actually move). Build them in any order.
- **Cutover order** — the migration script (Phase 2) is the single switch. It
  must run **after** the readers (Phases 3+4) understand the new layout, or a
  running server 404s every texture. Recommended sequence:
  **1 (write) → 3 (manifest) + 4 (server) → 2 (migrate + regen manifest +
  restart) → verify → 5 (client multi-map, later).**

### Phase 1 — pipeline write side

Introduce one small helper module (Python, `bin/lib/texpath.py`) that is the
single source of truth for the new shape, and route every writer through it:

```python
def variant_dir(id, dir="s", layer=0, variant=1) -> str   # "<id>.<dir>.<layer>/<variant>"
def map_file(map, ext="png") -> str                        # "<map>.<ext>"
def map_sibling(some_map_path, map) -> str                 # swap the map file in the same variant dir
```

Writers to convert (each currently builds names by string-suffix surgery on a
flat filename; each becomes "a fixed-named file inside the variant dir"):

| File | Symbol | Old behaviour | New behaviour |
|---|---|---|---|
| `bin/art` | `_find_diffuse` (60) | `find -iname '*.diffuse.png'` | find files named exactly `diffuse.png` (recursive) |
| `bin/art` | `_map_path` (67) | `<dir>/<base>.<type>.png` | sibling `<variant_dir>/<type>.png` |
| `bin/art` | grid/atlas cutter (~1312, 1357) | writes `<id>.diffuse.png`, `<id>.l.0.<v>.<map>.png` | writes `<id>.<dir>.<layer>/<v>/<map>.png` |
| `bin/art` | remaster/sheet split (~1446–1494) | writes `<id>.<dir>.<layer>.<v>.diffuse.png` | writes into the variant leaf |
| `bin/art` | `_parse_sheet_ref` (1240) | parses dotted `<id>[.<dir>][.<layer>][.<v>][.<map>]` CLI ref | keep accepting the dotted **shorthand** as user input; resolve it to the folder path |
| `bin/art` | Laigter loop (~521), `_flatten_normal_bg` (598) | per `*.diffuse.png` | per `*/diffuse.png` |
| `bin/lib/generate.py` | `template_name`/`sprite_name` (178), `load_template` (183), `load_hero_from_disk` (191) | `<id>.<dir>.<layer>.<v>.<map>.png` | folder form; update the docstring (7–12) |
| `bin/lib/split_layers.py` | `*_SUFFIX` (29), `find_albedos` (52), path build (152) | strip `.albedo.png`, append `.packed*.png` | siblings in the variant dir |
| `bin/lib/emissive.py` | `*_SUFFIX` (28), path build (91) | strip `.albedo_residual.png`, append `.emissive.png` | siblings in the variant dir |
| `marigold/delight.py` | `find_diffuse` (56), `out_path` (72) | `<base>.<target>.png` | `<variant_dir>/<target>.png` |
| `marigold/normals.py` | shares delight helpers | — | follows `out_path` |

**Watch-out — `delight.out_path`'s `out_dir` flatten.** It currently flattens a
spike-dir name as `f"{diffuse.parent.name}.{name}"`; after the move
`diffuse.parent.name` is just the variant number (`124`), which is **not unique**
across poses. Use the full relative path (or `id.dir.layer.variant`) as the
flatten key.

**Landed (2026-07-04).** New helper `bin/lib/texpath.py` (shape SoT for the
bin/lib scripts; marigold re-implements it natively in `delight.py`). Converted:
`split_layers.py`, `emissive.py`, `generate.py`, `marigold/delight.py` (→
`normals`/`depth`/`normal_depth` inherit), and `bin/art` — group-drop constants
(`SPRITE_DIR`/`MASTER_DIR`/`TEMPLATE_DIR` → `$TEX`), `_find_diffuse` (exact
`diffuse.png`), `_map_path` (leaf sibling), the grid slicer + remaster writers and
their cleanup globs, and the `art unmaps` deleter. The `out_dir` flatten uses
`<kind>.<pose>.<variant>` (leaf's last three components). Verified: py_compile +
`texpath`/`delight.out_path` unit tests + `bash -n` + a scratch-tree dry-run of
`_find_diffuse`/`_map_path`/cleanup globs (leaf `diffuse.png` found, flat
`<id>.diffuse.png` source atlas correctly ignored, `1.*/` cleanup hits only leaf
dirs). Prompts kept as loose `<seed>.prompt.txt` sidecars.

**Deferred out of Phase 1 (flagged):** `bin/art`'s manifest walk + `_kind_maps` +
`&variant`/`&layers`/`&subkinds` counters still assume flat names → **Phase 3**
(so `art manifest` misreports until then); R2 upload key + prune parser → **R2
step** (`art publish` inconsistent until then). Minor quirk: `art key` (debug) now
globs the unified tree and would key master leaves too — harmless (rel-path
preserved, scratch output), but noted.

### Phase 2 — migrate the existing tree (scripted `mv`)

A one-shot script (`bin/lib/migrate_texpaths.py`, or an `art migrate-paths`
subcommand) that restructures `textures/` in place. Algorithm per file:

1. Skip the loose files (sheets `*.sprite.png` at the category level, `*.alias`,
   `*.psd`, `*.prompt.txt` not tied to a variant) — leave them where they are,
   minus the group prefix.
2. For every `<group>/<cat…>/<kind…>/<stem>.<map>.png` where `<stem>` is a
   `id[.dir[.layer[.variant]]]` sequence:
   - parse the stem, applying the Phase-1 defaults for missing fields;
   - compute `textures/<cat…>/<kind…>/<id>.<dir>.<layer>/<variant>/<map>.png`;
   - `git mv` (preserve history) old → new.
3. Drop the now-empty `master/`, `sprites/`, `templates/` group dirs.

Edge cases the script must handle:
- **Sparse stems** (`1.diffuse.png`, `1.l.0.diffuse.png`) → padded to canonical.
- **Seed vs sequential variants** — no special-casing needed; the variant number
  is whatever the filename says. Wolf's `124` and a human's `0` both just become
  the `<variant>/` dir name.
- **Group collision on merge** — the same leaf may receive files from all three
  old trees (e.g. a `sprite.png` from `sprites/`, a `diffuse.png` from `master/`).
  That's the intended merge; assert no two sources produce the *same* map
  filename before moving.
- **Dry-run first** — print the full rename table for eyeball review before any
  `mv` (this is the "eyeball the mapping against the real tree" checkpoint).

After the moves: re-run `art manifest`, restart the server, verify a zone renders.

**Landed (2026-07-04).** Implemented as `bin/lib/migrate_texpaths.py` (dry-run by
default; `--apply` to execute). NON-DESTRUCTIVE: it **copies** into the new
group-less `<cat>/…` dirs and leaves `master/`/`sprites/`/`templates/` intact —
essential because **`textures/` is gitignored** (`.gitignore:85`), so the on-disk
old trees are the *only* rollback (git can't restore them). Applied on the real
tree: 541 files → **497 leaf + 43 loose + 1 anomaly (skipped), 0 collisions**, 540
copied. Verified the three groups **merge** per leaf (e.g. `world/conifer/1.s.0/0/`
= `diffuse/albedo/normal.png` from master + `sprite.png` from sprites; template
leaves get `template.png`+`template.psd`). Notes: the 45 `linked/wall.smooth`
`1.l.0.l.0.N` masters are pre-existing non-canonical (double-encoded stem) —
migrated **losslessly** via trailing-variant split to `1.l.0.l.0/N/`, but the
server/pipeline expect `1.l.0/N/`, so wall.smooth was already broken and should be
**re-mastered** post-migration. The 1 anomaly is `sprites/world/flora/1.s.0.0.png`
(no `.sprite` map token) — left in the old tree for manual triage.

**Rollback:** `rm -rf` the new top-level `textures/<cat>/` dirs; the old
`master/`/`sprites/`/`templates/` trees are untouched.

### Phase 3 — manifest generation

`content/visual/manifest/*.rd` carry per-kind `&maps` bits (bit0 albedo | bit1
normal | bit2 emissive), `&layers`, `&hash`, variation arrays — **not paths** — so
the manifest *format is unchanged*. Only the *detector* changes:

- `bin/art` `_kind_maps` (2069): today it globs `<kinddir>/*.albedo.png` etc.
  New: probe any `<kinddir>/**/albedo.png` (a variant subdir holding that map).
- The `art manifest` walk (comment at 2078) that assumes
  `…/<id>.<dir>.<layer>.<variant>.<map>.png` now descends into the `<variant>/`
  dirs. Variant counts (`&variant`) become a plain count of `<variant>/` subdirs
  under a pose — cleaner than globbing.

Regenerate and diff against the current `.rd` — the output should be identical
except for any counts the old glob got wrong.

**Landed (2026-07-04).** In `bin/art`: `_kind_maps` (exact `albedo.png`/`normal.png`/
`emissive.png` names, recursive), `_var_pairs` + `_layer_count` rewritten to walk the
`<id>.<dir>.<layer>/<variant>/` leaf dirs, and `cmd_manifest`'s category loop skips
the legacy `<group>` dirs kept for rollback. `_var_variant` was dead code (untouched);
`_kind_hash`/`_override_ints` work as-is. Also: `cmd_manifest` now **skips kinds with
no mastered variations** — the group-less tree co-locates source atlases + templates
with masters, so un-mastered kinds (`fence.*`, `rock.*`, `dead.*`, `male.thin`) would
otherwise appear as empty stubs. Verified: helper outputs match the old-tree baseline
(male.fit → `1 0/1 1/1 2`, 2 layers, maps=3); regenerated the real manifest → 4 files,
**8 mastered kinds, 81 variations** (`linked/wall.blueprint`, `world/{berry,conifer,
flora}`, `pawns.animal/wolf`, `pawns.human/male.{average,fat,fit}`). The committed
baseline was stale (berry 1–12 vs the tree's 0–11; pawns.*.rd didn't exist) — the
regen corrects it. `&hash` values change (leaf path-order), expected. `cmd_gc` (R2)
still needs the same legacy-group skip — folded into the R2 step.

### Phase 4 — server disk-path builders

Client stem contract is **unchanged** (`<cat>/<kind>[/<facing>]`); only the
stem→disk mapping moves. In `server/src/`:

- `tex_manifest.rs` `scan_masters` (141) — the hardcoded
  `1.{facing}.0.1.albedo.png` join becomes `1.{facing}.0/1/albedo.png`.
- `textures.rs` `master_albedo_rel` (185) — same join change; also the source
  root is now `textures/` directly (no `master/` prefix on disk), so build
  `<cat>/<kind>/1.<facing>.0/1/albedo.png`. **Keep the `/textures/master/…`
  HTTP route** — that's the serving tier (axis 2), not the disk group.
- `textures.rs` `derived_preview_path` (217) / `derived_lod_path` (224) — these
  live in the **cache** dir keyed by stem; unaffected by the source reshape
  except that they can keep appending `.albedo.png` (still albedo-only until
  Phase 5).

**Landed (2026-07-04).** `master_albedo_rel` now returns the group-less leaf
`<cat>/<kind>/1.<facing>.0/1/albedo.png` (shared by Disk + R2 reads, ETag, and the
manifest scan). `tex_manifest.rs::scan_masters` scans the textures root directly,
skips the legacy `<group>` dirs (`LEGACY_GROUPS`), and probes the leaf; tests updated
(`master_albedo_rel` assertions + `write_master`/serve-test write leaf paths). The
`/textures/master/…` HTTP route is unchanged (serving tier, not disk group). Verified
the migrated tree matches the probe path — `world/conifer/1.s.0/1/albedo.png` exists,
and kinds without a variant-1 albedo (wolf uses seeds; wall.smooth malformed) weren't
resolvable before either, so no regression. **Not compiled here** (no cargo/rustup in
this env) — the edits are mechanical + reviewed; build via the project toolchain
(Docker/`rd`) before deploy. **R2 caveat:** dropping `master/` from the shared rel
means R2 masters must be re-synced to the group-less layout before an R2 deploy — the
R2 step.

### Phase 5 — client multi-map fetch (deferred)

Only needed when the renderer's lighting pass wants `normal`/`emissive`
alongside `albedo`. Extend the URL + cache contract with a `<map>` dimension:

- `pixijs/src/textures/lod.ts` — `realUrl`/`previewUrl`/`lodUrl` gain a `map` arg
  (e.g. `/textures/lod/<hash>/<size>/<stem>/<map>` or `?map=normal`).
- `TextureResolver.ts` `ensureLod` (233) — fetch + cache per `(stem, size, map)`.
- `previewCache.ts` `lodKey` (29) + `LodBytes` (23) — key by map; store multiple.
- `textureManifest.ts` — the payload can advertise which maps exist per stem
  (fed by the `&maps` bits).

Until then the client stays albedo-only and this phase is untouched.

### R2 (folds into Phases 1–2)

R2 mirrors the disk tree under `$R2_PREFIX/textures/…`, plus derived
`lod`/`geo` tiers keyed by version hash:

- **Upload** (`bin/art` `_r2_sync_kind` ~2004): `master/<kind>` → key
  `$R2_PREFIX/textures/master/<kind>`. After the group drop, upload
  `<cat>/<kind>` → `$R2_PREFIX/textures/<cat>/<kind>` (decide whether to keep a
  `master/` key prefix for the serving tier or drop it too — recommend **keep**
  the serving-tier prefix so axis 2 is stable across disk and R2).
- **Stale prune** (`bin/art` ~2184): parses
  `$R2_PREFIX/textures/<group>/<cat>.<sub>/<kind>.<sub>/<version>/…`. This
  `<group>` is the *serving tier* (`lod`/`geo`) — it still holds. The
  `<cat>.<sub>/<kind>.<sub>/<version>/` shape is unchanged. No change needed
  unless we also drop the serving-tier prefix (we shouldn't).
- After Phase 2, re-sync R2 (`art publish`/`_r2_sync_kind`) so the remote tree
  matches, then prune stale versions.

**Landed (2026-07-04).** Resolved the open question **against** an R2 `master/`
prefix — dropping it everywhere (disk and R2 identical). Forced by Phase 4: the
server reads `{prefix}/textures/{rel}` and `rel` no longer carries `master/`, so
`cmd_upload_master` now uploads to `$R2_PREFIX/textures/<cat>/<kind>/…` (was
`…/textures/master/<cat>/<kind>/…`). The lod/geo **serving-tier** prefixes are
untouched (they're tiers, not the lifecycle group — the prune's `<group>` var is
really `<tier>` ∈ lod|geo; comment clarified). `cmd_gc`'s master-tree walk skips the
legacy `<group>` dirs. Not runnable here (needs docker + R2 creds) — verified only by
key-alignment: upload dir `…/textures/world/conifer/` ⊃ `1.s.0/1/albedo.png` = the
server's read path. Bloat note: the unified kind dir co-locates sprite/template/source
files; they upload too but the gate only reads albedo leaves.

---

## Decisions locked

- **Folder-per-variant**, variant as its own directory level (not a flat
  `<id>.<dir>.<layer>.<variant>/`) — makes variant enumeration a plain `readdir`.
- **Group dropped** — the three source trees merge; map filename disambiguates.
- **Serving tiers kept** — `master`/`preview`/`lod`/`geo` routes & cache dirs
  unchanged.
- **Normalize on write** — pipeline emits fully-qualified paths (defaults
  `dir=s`, `layer=0`, `variant=1`).
- **Scripted `mv`** migration (dry-run table first), not regenerate-from-source.

## Open questions

- **R2 serving-tier prefix**: keep `master/` as an R2 key prefix (recommended, so
  the serving-tier axis is identical on disk-cache and R2) or drop it to match the
  bare disk source? Affects `_r2_sync_kind` + the prune parser only.
- **Templates in the leaf**: a `template.png` has its own `<tvar>` variant,
  distinct from the sprite seeds it generates, so it lands in its own
  `<id>.<dir>.<layer>/<tvar>/template.png` rather than beside the sprites it
  seeded. Acceptable, but confirm we don't want templates kept in a separate
  authoring area.
