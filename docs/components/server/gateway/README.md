# Component — `server/gateway`

_Path: `server/gateway`. Deploys as: the client's first-hop directory. Last updated: 2026-07-14._

A thin directory over the `index` DB: the client asks `GET /server`, the gateway resolves a world
server (edge) and hands back its URL. Pure routing — it holds no game state.

- **[`intent/gateway.md`](intent/gateway.md)** — the two-hop login handshake, what it reads from
  `index`, entry/exit points.
- `design/`, `current/`, `plan/` — lazy (add when worked). Consumes the
  [`index`](../spacetime/modules/) module's directory tables.
