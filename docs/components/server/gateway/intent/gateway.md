# Gateway

The **gateway** is the entrypoint a client hits *before* the game. It does not
carry game state or a client stream — it answers one question: *which world
server should this player use?*

```
client ──GET /server?player_id=──▶ gateway ──reads──▶ index DB (servers, player_servers)
   │                                   │
   │◀──── { server: { url } } ─────────┘
   │
   └──login (claim_or_login)──▶ world server  ──becomes state-authoritative──▶ shards
```

1. The client asks the gateway for a server (optionally naming the player it
   already is, for reconnect).
2. The gateway resolves a world server from the `index` routing directory and
   returns its `url`.
3. The client logs into that world server. That server becomes
   state-authoritative for the player and bridges to the SpacetimeDB data shards.

The gateway's only upstream is the `index` database. It never talks to the data
shards (the `region_shard` module DBs) — that's the world server's job.

## Resolution

Implemented in `src/directory.rs`, with the selection policy isolated in
`src/resolve.rs` (`pick_server` — **the pluggable seam**). Two cases:

- **Affinity** — if `player_servers` already pins the player to a
  still-registered server, reuse it (so the world server can stream the session
  delta on reconnect instead of rebuilding it) and refresh the pin's activity
  stamp via `touch_player`.
- **Allocate** — otherwise pick a live server with `pick_server` (today: the
  freshest-heartbeat server — with one registered server, just "the server") and,
  for a known player, pin it via `assign_player` for next time.

Today every player resolves to the single registered server. To split players
across servers, change `pick_server` (e.g. route by the player's `data_shard`, or
pick least-loaded) — nothing else moves.

## HTTP API

| Route          | Result |
| -------------- | ------ |
| `GET /`        | hello banner |
| `GET /health`  | `200 ok` liveness (independent of the `index` connection) |
| `GET /server`  | resolve a world server |

`GET /server` accepts an optional `?player_id=<u32>` (reconnect affinity; omit or
`0` for a client with no established player yet). Responses:

- `200` → `{ "server": { "server_id": <u16>, "url": "<endpoint>" }, "reused": <bool> }`
- `503` → `{ "error": "directory unavailable" }` (index DB unreachable) or
  `{ "error": "no server available" }` (no world server registered).

## Configuration

One binary serves every env; it reads these at runtime (defaults target local dev):

| Var           | Default                        | Meaning |
| ------------- | ------------------------------ | ------- |
| `GATE_LISTEN` | `0.0.0.0:8080`                 | gateway HTTP bind address |
| `INDEX_URI`   | `http://127.0.0.1:3000`        | SpacetimeDB host holding `index` |
| `INDEX_DB`    | `resonantdust-dev-index-0`     | `index` database name |

Local gateway ports per env (see `gateway/compose.yml`): dev `8483`, claude
`8484`, test `8485` — distinct from the world server's `8473`/`8474`/`8475`.

## Build

```
rd build gateway              # compile the release binary (docker)
rd build gateway --bindings   # regenerate src/bindings/* from the modules, then build
```

## Registering a world server

The gateway only hands out servers that exist in the `index` `servers` table. A
world server makes itself eligible automatically: on startup it calls
`set_server(server_id, url, now_ms)`, then re-sends it every 20s as a heartbeat
(the index GC reaps a server whose heartbeat is older than `SERVER_TTL_MS` = 60s,
releasing its pinned players). See `Pool::spawn_registration` in
`server/src/connections.rs`. The advertised `url` is `SERVER_PUBLIC_URL`
(default `ws://localhost:8473/ws` for dev) and the id is `SERVER_ID` (default 0).

So just starting the world server is enough — no manual seeding. To confirm:

```
curl 'http://localhost:9473/server?player_id=1024'
# → {"server":{"server_id":0,"url":"ws://localhost:8473/ws"},"reused":false}
```

If you need to register a server *without* running it (e.g. pointing at a remote
host), seed by hand:

```
# now_ms = current epoch millis
spacetime call resonantdust-dev-index-0 set_server 0 "ws://localhost:8473/ws" $(date +%s%3N)
```
