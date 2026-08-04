# Plan — content is TOML-only

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), measurements in [`issues.md`](issues.md)._

## P0 — the inventory, frozen

- [x] Enumerate every tracked non-TOML path under `content/` with its writer and its grep-proven
      readers. Acceptance: every path from `git ls-files content | grep -v '\.toml$'` appears in
      `issues.md` with a verdict and a file:line per reader. → [I1](issues.md#i1): 7 paths, 3
      reader sites; two defects fell out ([I2](issues.md#i2), [I4](issues.md#i4)) plus a fossil
      walk ([I5](issues.md#i5)).

## P1 — the dead die first

- [x] Replace `load_r2`'s `manifest.json` fetch with a SigV4 `ListObjectsV2` over
      `<prefix>/content/`, taking `*.toml` keys sorted ([F4](forks.md#f4)). Acceptance: a unit
      test keeps `x.toml`, drops `manifest.json`, `y.rd` and a subdir key; `cargo check` green. →
      `r2_list_keys` (paginated, percent-decoded) + the pure `content_keys` filter;
      `content_keys_keeps_root_toml_only` passes, crate compiles.
- [x] Delete `content/manifest.json` and `bin/dsl`'s `reindex` command. Acceptance:
      `grep -rn manifest.json bin/ server/ shared/` finds nothing; `bin/dsl` usage lists no
      `reindex`. → both deleted; `bin/` is clean. Three hits remain in `content.rs` — two
      comments explaining the deletion and one test fixture asserting it's filtered out.
- [x] Delete the `.rd` facet fallback branch in `server/edge/src/content.rs` `load_disk`
      ([I5](issues.md#i5)). Acceptance: `load_disk` reads root `*.toml` only; the edge redeploys
      and `/content` still serves the 4 client TOMLs. → branch gone;
      `load_disk_reads_root_toml_sorted` proves root-only + `biomes.toml`/nested exclusion.
      Redeploy check deferred to the P2 commit (nothing else changed on that path).
- [x] Rename the `.rd` fixture names in `server/edge/src/content.rs` and
      `shared/content/src/content.rs` version tests to `.toml`. Acceptance: `cargo test -p
      resonantdust-edge -p resonantdust-content` green; no test names a dead dialect. → both
      renamed; edge's 5 content tests green, the shared workspace's 80 green incl. the golden
      fixture. The `/content` payload's `"rd"` KEY is a live wire contract — [F6](forks.md#f6)
      moves it to P4.
- [x] Port `bin/lib/def_span.py` to read `content/things.toml` ([I4](issues.md#i4)), keeping its
      span≠size≠footprint rules and pow2 round-up. Acceptance:
      `python3 bin/lib/def_span.py biome-thing/default/conifer` prints `2`; `--all` lists every
      def naming a texture. → both pass; an empty read now exits **2** and `bin/art` surfaces it,
      so I4's silent-zero can't recur.

## P2 — the art manifests leave `content/`

- [x] Make `bin/art manifest` emit `textures/manifest/<type>.json` ([F1](forks.md#f1),
      [F2](forks.md#f2)). Acceptance: regenerating all 3 types yields JSON whose decoded
      kind → {var_id, var_variant, layers, parts, subkinds, maps, hash} map equals today's `.rd`
      for every leaf. → 7 kinds / 82 variations, every field identical but one hash, which the
      unchanged `_kind_hash` reproduces from today's tree — the committed `.rd` was stale
      ([I6](issues.md#i6)). Regeneration is deterministic.
- [x] Delete `content/visual/manifest/*.rd` and the emptied `content/visual/`. Acceptance:
      `git ls-files content` lists only `*.toml` and `servers/*`. → confirmed;
      `textures/manifest/*.json` is untracked, as `textures/` already is.
- [x] Drop `CONTENT_FOLDERS=(visual)` from `bin/dsl` so the art index rides the texture upload,
      not the corpus one. Acceptance: `bin/dsl upload` syncs root `*.toml` only; its usage text
      names no folder argument. → the folder concept is gone entirely: `_remote_key_for`,
      `_upload_folder`, `_download_folder`, `_r2_sync`, `_r2_pull` deleted; `_check_relpath` now
      demands a flat `*.toml` name (both rejections verified).
- [x] Re-point `bin/art`'s stubbed publish tail ([bin/art:2700](../../../bin/art)) at
      `textures/manifest/`. Acceptance: the reference sequence in that comment names no `content/`
      path and no `dsl` corpus call. → rewritten; `bin/art`'s `manifest` usage text and the
      `cmd_manifest` header follow. [F7](forks.md#f7) logged: `bin/dsl` is itself a fossil name.

## P3 — the deploy topology leaves `content/`

- [x] `git mv content/servers deploy/servers` and re-point `rd_servers_manifest` in
      `bin/lib/common.sh` ([F3](forks.md#f3)). Acceptance: `bin/rd index show` for `dev` prints
      the same topology rows as before the move. → strengthened, because `show` reads the live DB
      and would pass regardless: `rd index seed` logs `← deploy/servers/dev` and seeds the same 4
      rows; `show` unchanged.
- [x] Update the path in `bin/rd` help, `bin/lib/index.sh` comments, and each of the four server
      files' own headers. Acceptance: `grep -rn 'content/servers' bin/ docs/ server/` finds
      nothing. → plus three the item didn't list: `docs/notes/tables.md`, the index module's
      `intent/README.md`, and `server/edge/src/config.rs`.

## P4 — the gate, so it stays true

- [ ] Add `rd content-check`: fail on any tracked file under `content/` not matching `*.toml`.
      Acceptance: exits 0 on the clean tree; exits 1 naming the path when `content/x.json` is
      staged.
- [ ] Call `content-check` from `.git/hooks/pre-commit` beside `docs-check`, sharing the
      `SKIP_DOCS_CHECK=1` escape hatch. Acceptance: a commit staging `content/x.json` is refused
      with the check's message; the same commit succeeds under the skip var.
- [ ] Rename `bin/dsl` → `bin/content` with its `bin/art` reference, the `dev/` list in
      `docs/CONVENTIONS.md`, and the `dsl-script-ported` memory ([F7](forks.md#f7)). Acceptance:
      `grep -rn 'bin/dsl' . --exclude-dir=.git` finds nothing; `bin/content --help` runs.
- [ ] Rename the `/content` payload key `"rd"` → `"toml"` in `server/edge`, `contentBoot.ts` and
      `client/npc/src/lib.rs` in one commit ([F6](forks.md#f6)). Acceptance: edge redeployed, the
      webgl client boots and renders the world, `rd-npc` resolves a def.
- [ ] Purge the stale `.rd` prose comments in `client/webgl` (`WorldBridge`, `Viewport`,
      `worldTilt`, `material`, `contentBoot`), `shared/wasm`, `client/npc`, and
      `server/edge/src/{config,worldgen}.rs`. Acceptance: `grep -rn '\.rd\b' client/ shared/
      server/ --include='*.rs' --include='*.ts'` names only history.
- [ ] Truth pass: `docs/CONVENTIONS.md`'s "`.rd` content specs" line, `docs/README.md`,
      `bin/dsl`'s header, and the `art-script-ported` / `dsl-script-ported` / `content-hot-update`
      memories. Acceptance: `bin/rd docs-check` green; no doc or memory names a live `.rd`.

## P5 — one texture index (the destination, [F5](forks.md#f5))

- [ ] Write the merge into `docs/components/server/edge/intent/`: `/textures-manifest` is the
      single index, and which 4 fields the art manifest still carries alone. Acceptance: the doc
      names each field, where it is generated, and who will read it.
- [ ] Extend the edge's `tex_manifest` scan with per-stem variant ids, layer count, part ids and
      subkinds. Acceptance: `/textures-manifest` reports the wolf's 15 variants and
      `biome-thing/default/conifer`'s parts; every existing field is byte-unchanged.
- [ ] Retire `bin/art manifest`'s standalone output once the edge index carries everything.
      Acceptance: `textures/manifest/` is gone, `bin/art manifest` is deleted from the dispatch,
      and no field listed in the P5 intent doc lost its reader.
