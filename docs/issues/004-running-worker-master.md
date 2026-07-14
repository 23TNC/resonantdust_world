# 004 — Running the worker/master binaries for a full-stack test (phase S3/S4)

## Problem

The worker and master are native binaries with **no host cargo** and **no build/run compose**
(unlike the edge/gateway). The old `rd_master`/`rd_worker` runners point at deleted paths (the
stale-binary problem the memory warns about). So the full stack couldn't be run to prove the
pipeline drives itself — only the module could be poked via `spacetime call`.

## Options

- **A · A throwaway rust+openssl container on `--network host`.** Build the binaries inside it
  (deps cached in the mounted `target/`), run them in the background pointed at a test DB.
- **B · Add real compose files** for worker/master (like edge) and an `rd run worker/master`.
- **C · Cross-compile static binaries** (musl + vendored openssl) runnable on the host directly.

## Choice — **A** (for the test); **B** is the real fix

`docker run -d --name rd-run --network host -v $PWD:/w rust:slim sleep infinity`, then
`apt-get install -y libssl-dev pkg-config ca-certificates`, `cargo build` worker + master, and
`docker exec -d ... ./target/debug/{master,worker}` with `ST_URI=http://127.0.0.1:3000` +
`SHARD_DB=<db>` (`--network host` reaches the `spacetime-start` container's :3000).

## Why

- **A unblocks the full-stack test immediately** with no repo changes — the link just needs
  `libssl-dev` (spacetimedb-sdk → native-tls), and `--network host` gives the binaries the DB.
- **B is the permanent fix** (proper dev-stack integration) but is infra work beyond the rewrite.
- **C** is best for deploy artifacts but overkill for a dev smoke test.

## Result — full stack verified live

Real master (2 Hz, driving `drop_timed_out → bump`) + real worker (two-phase loop + DSL
interpreter) against a fresh shard:
- `append` a SPAWN → the worker **autonomously** claimed → stood up → readied → claimed →
  resolved → entity in `state` (kind 5, loc 17). Worker log: `resolved ev=1 targets=1`.
- `append` a MOVE on it → kind **carried forward** (5), location → 34, rotation → east (the
  interpreter's `facing_from_delta`).

The whole hot-path pipeline runs end-to-end with the actual binaries. Follow-up: add worker/master
compose + `rd run` (B).


## Resolution — option B shipped as `rd run` (light form)

`bin/lib/run.sh` adds `rd run <up|worker|master|stop|status|logs>`. Rather than the heavier
edge-style compose+up/down wiring, it keeps ONE persistent `rd-run-<env>` container
(rust:slim + libssl-dev, repo bind-mounted, `--network host`), builds the debug binaries into
the mounted `target/`, and runs them detached against `resonantdust-<env>-zone-0`. Logs land at
`server/{worker,master}/{worker,master}-<env>.log`. **Verified:** `rd run up` on dev drove an
appended spawn to a resolved `state` row (worker log `resolved ev=1`), master ticking. This is
the self-driving half of the dev stack; edge/gateway remain `rd up`/`rd deploy`.
