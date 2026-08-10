# server/spacetime

The local SpacetimeDB stack: one pinned daemon, N module crates, driven by
[`bin/st`](../../bin/st).

```
compose.yml                 daemon + build + bindings services
modules/<module>/           one crate per database
  Cargo.toml                crate name → wasm name
  spacetime.json            anchor the CLI needs (empty {})
  spacetime.dev.json        db name + server for the dev env
  src/lib.rs                tables + reducers
scripts/generate-bindings.sh  module → server/edge/src/bindings/<module>/
keys/                       JWT signing pair          (gitignored, secret)
config/cli.toml             daemon-issued CLI token   (gitignored)
data/                       durable database state    (gitignored, root-owned)
```

## Start here

```bash
bin/st up && bin/st publish template && bin/st sql template "SELECT * FROM clock"
```

`up` creates the external `resonantdust` docker network and starts the daemon on
`http://127.0.0.1:3000`. `publish` builds the wasm locally, copies it into the
daemon, and publishes it to `resonantdust-dev-template-0`. The `clock` row's
`ticks` column climbing once a second is the stack's proof of life.

## Adding a module

Copy `modules/template/` to `modules/<name>/`, then:

1. `Cargo.toml` — rename the crate (`resonantdust_<name>`).
2. `spacetime.dev.json` — set `database` to `resonantdust-dev-<name>-0`.
   **No underscores in a database name** — the daemon rejects them. A module
   directory may be `event_shard`; its database is `...-event-shard-0`. `bin/st`
   applies that mapping for you.
3. `src/lib.rs` — replace the tables and reducers.

`bin/st build` with no argument builds every module found under `modules/`, so
there is no registry to update.

## Version pinning

The daemon image tag in `compose.yml` (`v2.3.0`) and the `spacetimedb` crate
version in every module (`=2.1.0`) are a matched pair. Bump them together or
publishing fails on module validation. `latest` is not safe: it currently
resolves to 2.5.0, whose `start` spins at 99% CPU and never initializes.

## Bindings

`bin/st bindings <module>` emits Rust client bindings into
`server/edge/src/bindings/<module>/` and appends the `pub mod` line to
`bindings/mod.rs`. Only public tables get a `*_table.rs` — the template's private
`clock` yields `clock_type.rs` and no table accessor, which is the visibility rule
showing up in the generated code.

Nothing generated is committed today (`server/edge/` is still a placeholder), so
regenerate after any schema change. The `rustfmt is not installed` warning from
the codegen is cosmetic: the files are written before formatting is attempted.

## uid 1000, and why `data/` breaks

The daemon image runs as `spacetime`, **uid 1000** — the same uid as the host
user here, which is the only reason a host-owned bind mount works at all.

Docker auto-creates a missing bind-mount target as `root:root`. So if `data/` does
not exist when the container starts, the daemon dies immediately with
`Permission denied ... /data/.tmpXXXX`. `bin/st up` pre-creates `data/` and
`config/` as the host user to prevent that, and warns if their owner doesn't match.

If you hit it anyway, `rmdir` the root-owned directory, then **remove the
container** before restarting — a container created against the old directory
holds a mount to the now-deleted inode and fails with `no such file or directory`
on start:

```bash
docker rm -f spacetime-start-1 && bin/st up
```

## Keys and identity

`keys/` holds the ECDSA pair the daemon signs JWTs with; `config/cli.toml` holds
a token issued under that key. They are a matched set — replacing one without the
other invalidates the CLI's identity and every database it owns. Both are
gitignored, so a fresh clone needs them supplied out of band (they carry over
from the 0.2.3 tree).

## Publishing wipes data

`bin/st publish <module>` passes `--delete-data=always`. There are no migrations
yet, and a schema change published against retained rows fails in confusing
ways, so the wipe is the default rather than the exception. `--keep` retains
data; use it only when the schema is genuinely unchanged.

One consequence worth internalising: `#[reducer(init)]` runs **only** on a fresh
publish. Anything it seeds — in the template, the scheduled `clock` row — is not
re-seeded by a `--keep` republish. If a schedule row is ever deleted out from
under you, the sweep is dead until a wiping publish re-runs `init`.
