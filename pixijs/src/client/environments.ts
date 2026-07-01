//! The **gateways** the client can log in through. Login is a two-hop handshake
//! (see `docs/gateway.md`): the client first asks a *gateway* (HTTP) which world
//! server to use, then connects to the world-server WebSocket the gateway hands
//! back. So the only endpoint this module models is the gateway — the world
//! server's URL is *discovered* at runtime from the gateway's `GET /server`
//! reply, never hardcoded here.
//!
//! Each env names a distinct gateway so the client can point at a LOCAL gateway
//! or the REMOTE (lightsail) one from the same login screen:
//!  - dev / claude / test → local gateways on the page's host, ports mirroring
//!    `bin/lib/common.sh`'s client-facing `RD_GATEWAY_PORT` (dev is the user's,
//!    claude the agent's, test the harness's).
//!  - alpha → the lightsail deployment at `gateway.resonantdust.com`.

export type Environment = "dev" | "claude" | "test" | "alpha";

/** Selectable environments, in display order. The login `Server` select is built
 *  straight from this list, so adding one here surfaces it in the UI. */
export const ENVIRONMENTS: readonly Environment[] = ["dev", "claude", "test", "alpha"];

/** A gateway endpoint. `host` omitted → the page's host (a local gateway served
 *  from the same machine); set → a fixed remote host. `secure` picks `https` vs
 *  `http`. */
interface GatewayEndpoint {
  host?: string;
  port: number;
  secure: boolean;
}

/** Per-env gateway endpoint. Ports mirror `bin/lib/common.sh`'s CLIENT-FACING
 *  `RD_GATEWAY_PORT` (9473/9474/9475 local) — distinct from the world server's
 *  8473/8474/8475, which the gateway resolves and returns. alpha is the lightsail
 *  host. Kept in step with the `rd` CLI and the Rust `client/config.rs`, which is
 *  the reference implementation of this same handshake. */
const GATEWAYS: Record<Environment, GatewayEndpoint> = {
  dev: { port: 9473, secure: false },
  claude: { port: 9474, secure: false },
  test: { port: 9475, secure: false },
  // Lightsail. Plain HTTP today (no TLS proxy) like the lightsail spacetime
  // server — flip `secure` to true and adjust `port` once a reverse proxy lands.
  alpha: { host: "gateway.resonantdust.com", port: 9473, secure: false },
};

/** The host for `env`: its fixed remote host, or the page's host for a local
 *  gateway. */
function hostFor(env: Environment): string {
  return GATEWAYS[env].host ?? (location.hostname || "localhost");
}

/** The gateway's HTTP base for `env` (`http(s)://host:port`, no trailing slash).
 *  The client issues `GET {base}/server` here to acquire a world server, and the
 *  debug HUD probes `{base}/versions`. Pass `host` to override the derived one. */
export function gatewayUrlFor(env: Environment, host = hostFor(env)): string {
  const { port, secure } = GATEWAYS[env];
  return `${secure ? "https" : "http"}://${host}:${port}`;
}

/** The env the client is currently connected to (set at login). Surfaced in the
 *  debug HUD + env badge so it's always unambiguous which gateway's data you're
 *  looking at — dev / claude / test / alpha. `null` until the first login. */
let _current: Environment | null = null;
const _listeners = new Set<(env: Environment | null) => void>();
export function setCurrentEnvironment(env: Environment): void {
  _current = env;
  for (const cb of _listeners) cb(env);
}
export function currentEnvironment(): Environment | null {
  return _current;
}

/** Subscribe to environment changes (login / server switch). Fires immediately
 *  with the current value so a freshly-mounted consumer (e.g. the env overlay)
 *  renders correctly before the first login. Returns an unsubscribe fn. */
export function onEnvironmentChange(cb: (env: Environment | null) => void): () => void {
  _listeners.add(cb);
  cb(_current);
  return () => _listeners.delete(cb);
}
