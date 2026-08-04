# shared/

The shared Rust — logic the **gateway**, the **spacetime modules**, and the
**pixijs client** all run from one source. It's called "shared" because the
server links it as **rlibs** while the client gets it as a **wasm bundle**: same
code, both sides. That single shared evaluation is the point.

## Layout

A pure Cargo workspace: `Cargo.toml` here is just the `[workspace]` member list,
and **one folder per shared piece**. Each new responsibility (codec / state /
protocol / rules …) gets its own folder crate; add it to `members` in
`Cargo.toml`.

- **`core/` — `resonantdust-core`.** Hello-world placeholder for the foundational
  shared logic. Pure Rust, no wasm — the gateway and modules link it directly.
- **`content/` — `resonantdust-content`.** The content loader: `content/*.toml`
  → the materialized `Bundle` every consumer queries (schema in
  `docs/VARIABLES.md`; the `.rd` DSL it replaced is deleted — git has it).
  Pure Rust, no wasm, same as `core`.
- **`wasm/` — `resonantdust-shared` (cdylib + rlib).** The browser bundle: thin
  `wasm-bindgen` wrappers that re-export the piece crates, gated on the `js`
  feature so `cargo test` / `check` exercise the logic natively. This is the one
  crate the pixijs client imports (via `pkg/`); the server never consumes it.

## Build & test

All dockerized on the `clockworklabs/spacetime` image (host `cargo` is not used):

| Command | Action |
| --- | --- |
| `docker compose run --rm test`  | `cargo test --workspace` |
| `docker compose run --rm check` | `cargo check --workspace --all-targets` |
| `docker compose run --rm build` | native release build of every crate |
| `docker compose run --rm wasm`  | the wasm bundle (`cargo build --target wasm32 --features js` + `wasm-bindgen`) → `pkg/` |
