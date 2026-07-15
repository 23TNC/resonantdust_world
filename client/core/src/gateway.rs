//! The gateway round-trip: `GET {gateway_url}/server` to acquire a world server.
//!
//! This is the *first* step of login — before touching a world server the client
//! asks the gateway which one to use (see `docs/components/server/gateway/intent/gateway.md`). The reply is the
//! `{ server: { server_id, url }, reused }` envelope the gateway serializes; we
//! flatten it into an [`ServerInfo`](crate::ServerInfo). A non-2xx response
//! (`503` when the directory is down or no server is registered) becomes an
//! `Err` carrying the gateway's `{ "error": … }` message.
//!
//! Two layers, so each transport shares the parsing and differs only in *how* it
//! issues the GET:
//!   - [`resolve_url`] + [`parse_resolve`] — pure, host-agnostic (status + body
//!     in, [`ServerInfo`] out). The wasm engine drives these around `fetch`.
//!   - [`resolve_server`] — the native (`reqwest`) round-trip, gated on `native`.

use crate::ServerInfo;

/// The gateway's `GET /server` success envelope.
#[derive(serde::Deserialize)]
struct ResolveResponse {
    server: ServerEndpoint,
    reused: bool,
}

/// The nested `server` object: the world endpoint to connect to.
#[derive(serde::Deserialize)]
struct ServerEndpoint {
    server_id: u16,
    url: String,
}

/// The gateway's error envelope (`{ "error": "…" }`), used to surface a readable
/// reason from a `503`.
#[derive(serde::Deserialize)]
struct ErrorResponse {
    error: String,
}

/// Build the resolve URL. `player_id` carries reconnect affinity when known (the
/// gateway reuses the player's pinned server); pass `None` (or `0`) on a
/// first-ever connect with no established player yet.
pub fn resolve_url(gateway_url: &str, player_id: Option<u32>) -> String {
    let base = gateway_url.trim_end_matches('/');
    match player_id.filter(|&p| p != 0) {
        Some(pid) => format!("{base}/server?player_id={pid}"),
        None => format!("{base}/server"),
    }
}

/// Parse a gateway resolve response — the pure half shared by every transport.
/// `status` is the HTTP status, `body` the response text. A non-2xx surfaces the
/// gateway's `{ "error": … }` (falling back to the raw body / status).
pub fn parse_resolve(status: u16, body: &str) -> Result<ServerInfo, String> {
    if !(200..300).contains(&status) {
        let detail = serde_json::from_str::<ErrorResponse>(body)
            .map(|e| e.error)
            .unwrap_or_else(|_| body.to_string());
        return Err(format!("gateway returned {status}: {detail}"));
    }

    let parsed: ResolveResponse =
        serde_json::from_str(body).map_err(|e| format!("unreadable gateway response: {e}"))?;

    Ok(ServerInfo {
        server_id: parsed.server.server_id,
        url: parsed.server.url,
        reused: parsed.reused,
    })
}

/// Ask the gateway for a world server over the native (`reqwest`) transport.
/// Issues the GET, then delegates to [`parse_resolve`].
#[cfg(feature = "native")]
pub async fn resolve_server(
    gateway_url: &str,
    player_id: Option<u32>,
) -> Result<ServerInfo, String> {
    let url = resolve_url(gateway_url, player_id);

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("gateway request to {url} failed: {e}"))?;

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    parse_resolve(status, &body)
}
