//! Server-selection policy — **the pluggable seam**.
//!
//! Given the live server pool from the `index` directory, decide which world
//! server a player should be handed. Today every player goes to the one
//! registered server, so this is just "pick the live server"; the function
//! exists so the policy that splits players across servers (least-loaded, or
//! routing by the player's `player_shard_reference`) lands in exactly one place
//! without touching the request plumbing in [`crate::directory`].

use crate::bindings::index::Server;

/// Choose which live world server to allocate to a player from the registered
/// pool. Returns `None` when no server is registered (the gateway then answers
/// `503` — there's nothing to log into yet).
///
/// Selection today picks the **freshest heartbeat** (`max last_seen_ms`). With a
/// single registered server that's simply "the server"; with several it prefers
/// the one most recently alive, a reasonable default until a real load metric
/// exists. The index GC reaps servers whose heartbeat goes stale, so rows in the
/// pool are live within the GC window.
///
/// To split players across servers, replace the body here — e.g. take the
/// player's `player_shard_reference` and index a shard→server map, or pick the least-loaded.
pub fn pick_server(pool: &[Server]) -> Option<&Server> {
    pool.iter().max_by_key(|s| s.last_seen_ms)
}
