# Client

The **client** is the game client as a *headless* Rust library — all logic, no
rendering. It talks to the gateway and a world server and holds the session;
`pixijs` will later become a dumb display layered over it (via a wasm/FFI shim),
and other Rust programs can drive it the same way. There is one contract for
every host: send **commands** in, render **events** out.

```
host (pixijs / a Rust program / the `headless` CLI)
   │  Command::Login { name }                         Event::LoggedIn { … }
   ▼                                                        ▲
 Client handle ──▶ engine task ──GET /server──▶ gateway     │
                        │                                    │
                        └──WS /ws──▶ world server ──login────┘
```

## The host API (`src/api.rs`)

The seam that lets "stuff call into" the client — pure data, no transport, so it
serves a native driver today and the pixijs wasm bridge later:

- **`Command`** — driven *in*: `Login { name }`, `Logout`, `Shutdown`.
- **`Event`** — emitted *out*: `LoginStarted`, `ServerResolved`, `LoggedIn`,
  `LoginFailed`, `Disconnected`, `Status`.
- **`EventSink`** — a host supplies one to receive events. Any
  `Fn(Event) + Send + Sync` is a sink, so a closure (forwarding to a channel, or
  calling into JS) is the usual choice.

```rust
use client::{Client, ClientConfig, Command, Event};

let client = Client::spawn(ClientConfig::from_env(), |event: Event| {
    println!("{event:?}");
});
client.login("Alice")?;          // == client.send(Command::Login { … })
```

`Client::spawn` launches one engine task (`src/engine.rs`) that owns all session
state and `select!`s the command channel against the world-server read stream —
no locks. It needs a tokio runtime (the `native` feature, on by default).

## Login flow

`Command::Login` runs the two-hop handshake from `docs/gateway.md`:

1. **Gateway** — `GET {gateway_url}/server` (with `?player_id=` for reconnect
   affinity once a player is established) → `{ server: { server_id, url }, reused }`.
   → `Event::ServerResolved`.
2. **World server** — open the WebSocket at the returned `url` and send
   `{"t":"login","cid":…,"client_time_ms":…,"name":…}`. The reply
   `{"t":"login_ok",…}` → `Event::LoggedIn { player_id, data_shard, … }`;
   `{"t":"login_err",…}` → `Event::LoginFailed`.

The wire frames live in `src/protocol.rs` — the client-side mirror of
`server/src/protocol.rs` (keep the two in lockstep until the protocol moves to a
shared crate).

## Configuration

The client's only static endpoint is the gateway's HTTP base; the world-server
url and data shard are discovered from the gateway reply. Resolved at runtime,
most specific first:

| Var                  | Meaning |
| -------------------- | ------- |
| `CLIENT_GATEWAY_URL` | explicit gateway base, e.g. `http://127.0.0.1:9473` |
| `RD_GATEWAY`         | the `bin/rd` CLI's client-facing gateway for the active profile |
| `CLIENT_ENV`/`RD_ENV`| env tag selecting a default (`dev`→`:9473`, `claude`→`:9474`, `test`→`:9475`, `alpha`→lightsail) |

## The `headless` binary

A minimal CLI host (`src/bin/headless.rs`): log in and print every event. Smoke
tests the round-trip without pixijs and doubles as a worked driver example.

```
headless <name>                                # configured gateway
RD_ENV=claude headless Bob                      # the claude env's gateway
CLIENT_GATEWAY_URL=http://127.0.0.1:9473 headless Bob
```

It stays connected after a successful login (so disconnects surface) until
Ctrl-C; a failed login exits non-zero.

## Build

All current envs are plain `ws://` / `http://`, so the client pulls in **no TLS
backend** — stock `rust:slim` builds it (no openssl/pkg-config), mirroring
`gateway/compose.yml`. Add a TLS feature in `Cargo.toml` when an env goes `wss://`.

```
docker compose -f client/compose.yml run --rm check    # cargo check --all-targets
docker compose -f client/compose.yml run --rm build    # release binary
docker compose -f client/compose.yml run --rm \
  -e CLIENT_GATEWAY_URL=http://gateway:9473 headless Alice
```
