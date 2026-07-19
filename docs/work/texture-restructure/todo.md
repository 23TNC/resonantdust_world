# Todo — texture-restructure

_Phased so nothing breaks mid-migration: the new write path + registry land first, then a **copy**
(not move) migration, then readers, then verify + drop the old tree. Items move to
[`completed.md`](completed.md) as they land + verify (`todo → completed`). Design:
[`README`](README.md) · decisions [`forks.md`](forks.md) · blockers [`blockers.md`](blockers.md)._

_All items 2026-07-19._

---

## P0 · The `type/subtype` `meta.json` registry

- [ ] **Schema** — define `textures/<type>/<subtype>/meta.json`: the **kind registry** (`kind` name →
      `kind_id`, carrying the `0x800` tile/linked flag), the **form → `variant_id`** map (linked), and
      **sheet-split** info (which source sheet a kind/variant came from — the metadata we dropped when
      type/subtype sheets went away). See [forks F2](forks.md#f2).
- [ ] **Author** it for every `type/subtype` on disk today: `biome-tile/default`, `biome-thing/default`,
      `pawn/animal` (+ any others a `find textures -maxdepth 2 -type d` turns up). Seed the linked
      materials (`smooth`/`brick`/`plank`/`metal`/`flecked`/`blueprint`…) into the `≥0x800` half.
- [ ] Decode helper (Python side for `bin/art`; Rust/TS side later reads it via the manifest).

## P1 · `bin/lib/texpath.py` — the shape SoT  _(leaf reshape ✅ → [completed.md](completed.md); remain:)_

- [ ] **Biome-tile fold** in `texpath` — given a linked source (`<form>.<material>`), emit
      `biome-tile/<biome>/<material>/<form>/…` (material→kind, form→variant via the **P0 registry**).
      Sequenced after P0 (needs the registry to resolve form→variant).
- [ ] **Mirror** the new leaf composition in `marigold/delight.py` (separate venv, re-implements texpath).

## P2 · `bin/art` write + manifest sites

- [ ] Every **write** site emits the new leaf: `split_layers.py`, `generate.py`, `emissive.py`,
      `marigold/delight.py`, `meta.py` (the leaf `meta.json`).
- [ ] The **manifest walk** (`_kind_maps`, variant/part counters) globs the new leaf; **truncate
      `variant_id ≥ 16`** out of the manifest (`u4`) — and `log()` what was dropped (no silent cap).
- [ ] Manifest entries lose `<subkind>`, gain the `biome-tile` prefix for former-linked kinds.

## P3 · Scripted copy migration of the existing tree

- [ ] A fresh migration script (the prior 0.1→0.2 migration in **git history** is the precedent):
      **dry-run table first**, then `--apply`. **Copy, don't move** (`textures/` gitignored → old tree = rollback).
- [ ] Two transforms: (a) thing/tile leaf reshape + drop subkind; (b) linked fold + kind↔material invert,
      **carrying the held-whole atlas + `atlas.json`** and **dropping** the per-cell `1..16` folders ([F3](forks.md#f3)).
- [ ] **Re-master to an atlas** the kinds not yet held-whole — `wall.blueprint` (still 16-split), the empty
      `fence.*`/`rock.*` — plus the known-broken `wall.smooth` double-encoded `1.l.0.l.0` masters. Disk is
      half-migrated, so P3 is per-kind, not a uniform rename.

## P4 · Server resolvers (edge)

- [ ] `server/edge` `textures.rs` (`master_albedo_rel`) + `tex_manifest.rs` (`scan_masters`) build/probe
      the new leaf (`…/<kind>/<variant>/<map>.<dir>.<part>.png`, no subkind).
- [ ] The `/textures/meta/{stem}` serve path resolves the leaf `meta.json` under the new shape.

## P5 · Client fetch/cache contract

- [ ] `client/pixijs/src/textures/*` (`TextureResolver`, `lod.ts`/`metaUrl`, `MaxRectsPacker`,
      `previewCache`, `textureManifest`): fetch URL + cache key gain `<dir>.<part>`, lose `<subkind>`;
      former-linked resolve under the `biome-tile/…` prefix.

## P6 · Verify + retire

- [ ] End-to-end on the running stack (`rd up` → `rd deploy` → browser renders a thing + a tile + a
      wall). Path change → unit tests can't close it; the browser is the gate.
- [ ] Drop the **old** leaves once satisfied (the copy's originals).

---

**Done when:** `textures/` is `<type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>` end to end
— walls/fences/rocks under `biome-tile/…`, no `<id>`/`<subkind>`, the registry authoritative, art +
edge + client all on the new leaf, browser-verified, old tree dropped.
