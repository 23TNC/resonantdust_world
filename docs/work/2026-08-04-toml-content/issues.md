# Issues — TOML content

## I1 — the P0 inventory: features used, consumers counted {#i1}

**Ops actually in corpus CODE** (comments stripped; 2026-08-04): `set` 373 · `return` 97 ·
`call` 39 (= `^prim` 19 + `^rand` 9 + `^biome` 6 + the dead `&tile.destroy call` stubs) ·
`export` 19 (the `^prim` pattern) · `lt` 13 / `ge` 5 / `le` 1 · `if` 9 (all biome scatter) ·
`and` 4 · `drop` 5 (destroy stubs). **Zero** `or`, `not`, loops, `@on_update`, cross-def
reads — the earlier grep that showed them was matching PROSE in comments. The README's
"four features" claim verifies; the behavioral surface is even narrower than planned
(conjunctions only — no `or` to encode).

**Every Bundle/loader consumer, by load entry:**
- `shared/wasm/src/lib.rs:84` (+ a test at 921) — the client's `Content` (`dsl::load(&pairs)`
  from `/content`); also `needs_eval` at 242/255/256/266 and `VisualPart` consumers 428/476/572.
- `server/edge/src/worldgen.rs:62–82` — `read_content_dir` + `load` (worldgen classify AND the
  `/content` serving source); `content_version` also at `connections.rs:150`.
- `server/worker/src/main.rs:97` — disk-mirror corpus → speeds table.
- `client/npc/src/lib.rs:210` — `fetch_corpus` (HTTP `/content` → Bundle; Brain keeps it).
- `bin/dsl` — R2 publisher of `content/` (folders listed in load order at its head).
- `bin/lib/subframe.py` — EMITS `.rd` lines (generator, must emit TOML in P5).
- NOT consumers (verified absent): `client/core`, gateway, master, orchestrator, spacetime
  modules — codec only.

**Dead on arrival:** the 17 `@on_destroy` stubs (`0 return` / `&tile.destroy call drop` —
the handle is never defined anywhere) and 5 `drop`s die with the hooks; nothing reads them
(`tile_has_hook("…","on_destroy")` has zero call sites).
