//! The gate-served content boot surface.
//!
//! The tile content (the DSL corpus under `content/`) is decoded through the
//! `shared` wasm runtime into a {@link Content} bundle the renderer queries per
//! zone (`zoneTilePrims`). [`loadContent`] builds it from the `.rd` sources,
//! embedded at build time via Vite `?raw` (data facets before visual, the order
//! the loader needs so `def_id`s match the server's).
//!
//! The locale half is still a stub:
//!   - `sharedLocales()` — `panelStrings` queries it first, falling back to the
//!     bundled English copy (`content/locales/panels/en.json`) when it returns
//!     null. With no gate content, every lookup returns null → bundled English.
//!   - `getContentVersion()` — the debug HUD's version row; "" until content loads.

import { Content } from "../../client/wasm";
// The DSL tile corpus, embedded as raw strings. Data facets first (they own the
// `def_id` numbering), then visual (the colours fold on). Mirrors the server's
// `content/data` then `content/visual` read order.
import dataTiles from "@content/data/tiles.rd?raw";
import visualTiles from "@content/visual/tiles.rd?raw";

/** Build the tile {@link Content} bundle from the embedded `.rd` corpus. Must be
 *  called after {@link initWasm}. Throws if the corpus fails to parse. */
export function loadContent(): Content {
  const names = ["data/tiles.rd", "visual/tiles.rd"];
  const sources = [dataTiles, visualTiles];
  return new Content(names, sources);
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

/** The loaded content corpus version (debug HUD). Empty until content loads. */
export function getContentVersion(): string {
  return "";
}
