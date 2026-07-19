# `client/core` — the headless game client (`client`)

The game client as a **headless Rust library**: all logic, no rendering. It talks to the gateway
and a world server, holds the session, and exposes one contract for every host — send **commands**
in, receive **events** out. `pixijs` is a display layer over it (via the wasm shim in
`shared/wasm`); the `headless` binary and the `npc` driver are the same contract with no browser.

- **[`intent/`](intent/)** — [`client.md`](intent/client.md): the shape + role, the host API
  (`src/api.rs`), login flow, configuration, the `headless` binary. [`sync.md`](intent/sync.md):
  the synced-clock / render-delay `D` / deterministic-projection model this client feeds.
- `design/`, `current/`, `plan/` — lazy, per [CONVENTIONS](../../../CONVENTIONS.md); they come when
  this component is next worked.

Related: [`client/pixijs`](../pixijs/) (the browser display over this).
