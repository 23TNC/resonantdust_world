# Resonant Dust — world (0.3.0)

A multiplayer world game on a SpacetimeDB backend. **0.3.0 is a rewrite**: the
0.2.3 tree was removed wholesale and the layout below is a scaffold, not a
port. Prior art is on the `0.2.3` branch (`git show 0.2.3:<path>`) and in the
working copy at `../resonantdust_world_old` — read from either freely, but
nothing carries over by default.

## Layout

```
client/                 the browser client
server/
  edge/                 client-facing process (ws, subscriptions, worldgen)
  gateway/              entrypoint; routes a player to a server
  master/               authoritative clock
  orchestrator/         event framing + promotion
  spacetime/            SpacetimeDB stack: daemon + module crates  ← populated
  worker/               event execution
bin/st                  the SpacetimeDB driver
```

Only `server/spacetime/` has content: a pinned daemon, a module template, and the
`bin/st` driver. Every other folder is an empty placeholder — a `.gitkeep` is all
that's in it.

## The one thing that exists

```bash
bin/st up && bin/st publish template && bin/st sql template "SELECT * FROM clock"
```

See [server/spacetime/README.md](server/spacetime/README.md) for the module
workflow, the daemon/crate version pinning, and why a publish wipes data.

## Notes for the rewrite

- **Database names take no underscores.** The daemon rejects them. A module
  directory may be `event_shard`; its database is `resonantdust-dev-event-shard-0`.
- **Docker and uid 1000.** The spacetime image runs as uid 1000, matching the host
  user, which is the only reason host-owned bind mounts work. But docker
  auto-creates a *missing* mount target as root, and other images (`rust:slim`)
  run as root outright — so build output can land root-owned and resist `rm -rf`
  from the host. Clear that from a container: `docker run --rm -v $PWD:/repo
  busybox rm -rf /repo/<path>`.
- **The docs-authority system is gone**, along with `bin/rd`, `docs/`, `content/`,
  and `shared/`. Reintroduce what earns its place; don't restore it wholesale
  because 0.2.3 had it.
