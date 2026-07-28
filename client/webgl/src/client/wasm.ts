//! The Rust→wasm bundle (`shared/pkg`, built by `bin/rd build shared`) — loaded
//! once and re-exported for the rest of the client.
//!
//! `resonantdust-shared` wraps the shared Rust logic for the browser: the
//! [`WorldClient`] (login + the anchor-driven zone subscription engine, the
//! `client` crate's `web` transport) and [`Content`] (the DSL runtime that turns
//! a packed zone into renderable tile prims). Both need the wasm module
//! instantiated first, so callers must `await` {@link initWasm} before
//! constructing either — `main.ts` does this once at boot.

import init, { WorldClient, Content, ticHz, ticsPerTile } from "@shared/resonantdust_shared.js";

let ready: Promise<void> | null = null;

/** Instantiate the wasm module (idempotent — the first call loads it, later
 *  calls await the same promise). Resolves once `WorldClient` / `Content` are
 *  safe to construct. */
export function initWasm(): Promise<void> {
  if (!ready) {
    ready = init().then(() => undefined);
  }
  return ready;
}

export { WorldClient, Content, ticHz, ticsPerTile };
