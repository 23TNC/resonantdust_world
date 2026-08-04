//! The gate-served content boot surface.
//!
//! The content (the DSL corpus under `content/`) is decoded through the `shared`
//! wasm runtime into a {@link Content} bundle the renderer queries per zone
//! (`zoneTilePrims` for ground, `zoneThingPrims` for the scattered flora).
//!
//! **Runtime-served, hot-swappable.** [`loadContent`] fetches the corpus from the
//! world server's `GET /content` (`{ version, rd: [[name, text], …] }`, data facets
//! before visual so def-ids match the server). [`startContentPolling`] then polls
//! `GET /content-version`; when the server's fingerprint moves (an author edited a
//! `.rd` and `dsl upload`ed, or a dev edited the bind-mounted tree),
//! [`reloadContent`] re-fetches, rebuilds the wasm `Content`, frees the old, and
//! fires [`onContentReloaded`] — so subscribers (the world renderer) repaint with
//! the new defs, no page reload.
//!
//! **Embedded fallback.** The same `.rd` files are also embedded at build time via
//! Vite `?raw`. The server isn't known until login, so the app BOOTS from the embed;
//! on login the server's corpus is pulled and hot-swapped in (and it's the fallback
//! again if the server is unreachable). The version poll reconciles either way.
//!
//! The locale half is still a stub:
//!   - `sharedLocales()` — `panelStrings` queries it first, falling back to the
//!     bundled English copy when it returns null. Empty here → bundled English.
//!   - `getContentVersion()` — the debug HUD's live content row.

import { Content } from "../../client/wasm";
import { assertLinkedCellTable } from "../world/linkedCell";

// build-walls P0 (D1): pin the neighbor→cell formula against the authored 16-row table at
// boot — 16 comparisons, throws on drift, so a wrong formula can never ship silently.
assertLinkedCellTable();
// The TOML corpus embedded as raw strings — the OFFLINE FALLBACK only (the server's
// `/content` is the source of truth; ids are explicit, so order is immaterial —
// toml-content P6). `biomes.toml` is server-only worldgen and stays out, matching
// what `/content` serves.
import tilesToml from "@content/tiles.toml?raw";
import thingsToml from "@content/things.toml?raw";
import materialsToml from "@content/materials.toml?raw";
import needsToml from "@content/needs.toml?raw";

/** The server's `/content` payload: a version fingerprint plus the ordered
 *  `[name, text]` source pairs the client feeds to `new Content(names, sources)`.
 *  (`rd` is the WIRE KEY's historical name — a literal, like the `/lod/` route.) */
interface ContentPayload {
  version: string;
  rd: [string, string][];
}

/** The build-time embed, used only when the server is unreachable. `version` is
 *  a sentinel distinct from any served fingerprint, so the first successful poll
 *  always reconciles a fallback boot up to the server's real corpus. */
const EMBEDDED: ContentPayload = {
  version: "embedded",
  rd: [
    ["materials.toml", materialsToml],
    ["needs.toml", needsToml],
    ["things.toml", thingsToml],
    ["tiles.toml", tilesToml],
  ],
};

/** The live corpus + its version. `content` is null until {@link loadContent}. */
let content: Content | null = null;
let contentVersion = "";

/** Listeners fired after a hot-swap (see {@link reloadContent}). */
const reloadListeners = new Set<() => void>();

/** Build a wasm {@link Content} from a payload's `rd` pairs. Throws if the corpus
 *  fails to parse (a bad gate corpus → caller keeps the live one). */
function buildFromPayload(payload: ContentPayload): Content {
  const names = payload.rd.map(([name]) => name);
  const sources = payload.rd.map(([, text]) => text);
  return new Content(names, sources);
}

/** Build `payload` into a new `Content` and swap it in, freeing the old wasm
 *  bundle. Built-before-freed, so a throw leaves the live corpus untouched. */
function swapTo(payload: ContentPayload): void {
  const next = buildFromPayload(payload);
  content?.free();
  content = next;
  contentVersion = payload.version;
  // Debug affordance (mirrors `__client`): the live corpus, reachable from the console.
  (globalThis as unknown as { __content: Content }).__content = next;
}

/** Fetch + parse the gate's `/content`. Throws on a non-OK response. */
async function fetchContent(gatewayUrl: string): Promise<ContentPayload> {
  const resp = await fetch(`${gatewayUrl}/content`);
  if (!resp.ok) throw new Error(`/content ${resp.status} ${resp.statusText}`);
  return (await resp.json()) as ContentPayload;
}

/** Load the corpus at boot: fetch it from `gatewayUrl` (falling back to the
 *  embedded copy if the gate is unreachable or serves an unparseable corpus).
 *  Must be called after `initWasm`. Returns the live {@link Content}. Pass no url
 *  to boot straight from the embed (e.g. a content-less test). */
export async function loadContent(gatewayUrl?: string): Promise<Content> {
  if (gatewayUrl) {
    try {
      swapTo(await fetchContent(gatewayUrl));
      return content!;
    } catch (err) {
      console.warn("[content] gateway fetch failed; using embedded corpus", err);
    }
  }
  swapTo(EMBEDDED);
  return content!;
}

/** Re-fetch `/content` and hot-swap if its version differs from the live one.
 *  Rebuilds the wasm `Content`, frees the old, and fires {@link onContentReloaded}.
 *  Returns whether it swapped. A parse error on the new corpus propagates with the
 *  live corpus intact. */
export async function reloadContent(gatewayUrl: string): Promise<boolean> {
  const payload = await fetchContent(gatewayUrl);
  if (payload.version === contentVersion) return false;
  swapTo(payload);
  for (const cb of reloadListeners) cb();
  return true;
}

/** Subscribe to content hot-swaps — fired after `Content` is replaced, so a
 *  listener re-derives content-derived state (the world renderer repaints). */
export function onContentReloaded(cb: () => void): () => void {
  reloadListeners.add(cb);
  return () => reloadListeners.delete(cb);
}

/** The live content bundle. Throws if called before {@link loadContent}. */
export function getContent(): Content {
  if (!content) throw new Error("content not loaded — await loadContent() first");
  return content;
}

/** The version poll timer, if running. */
let pollTimer: number | null = null;

/** Start polling the gate's cheap `/content-version` for hot-swaps. `gatewayUrl`
 *  is read fresh each tick (a getter) so the poll follows the client if it repoints
 *  to another env at login. On a version that differs from the live corpus,
 *  {@link reloadContent} refetches + swaps. Idempotent — replaces any running poll. */
export function startContentPolling(gatewayUrl: () => string, intervalMs = 3000): void {
  stopContentPolling();
  pollTimer = window.setInterval(() => {
    const base = gatewayUrl();
    if (!base) return;
    void (async () => {
      try {
        const resp = await fetch(`${base}/content-version`);
        if (!resp.ok) return;
        const version = (await resp.text()).trim();
        if (version && version !== contentVersion) await reloadContent(base);
      } catch {
        /* gate unreachable — retry next tick */
      }
    })();
  }, intervalMs);
}

/** Stop the version poll (no-op if not running). */
export function stopContentPolling(): void {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
}

/** Minimal locale catalog: a key→string lookup. The real catalog is gate-served
 *  and language-switchable; this stub has no entries, so every key misses. */
export interface SharedLocales {
  string(key: string): string | null;
}

const EMPTY_LOCALES: SharedLocales = {
  string: () => null,
};

/** The active locale catalog. Always empty in the stub, so `panelStrings`
 *  resolves chrome text from its bundled English fallback. */
export function sharedLocales(): SharedLocales {
  return EMPTY_LOCALES;
}

/** The live content corpus version (debug HUD). Empty until content loads; the
 *  gate fingerprint (or `"embedded"` on the offline fallback) once it has. */
export function getContentVersion(): string {
  return contentVersion;
}
