//! Persistent texture-byte store (IndexedDB) — pins each stem's map bytes in a store the
//! browser's HTTP cache can't evict, so a returning player packs the whole world with
//! ZERO network (one-resolution: a fresh reload after priming issues no texture fetches).
//!
//! Keyed by `stem@map`, one entry per (stem, MAP) at the manifest's one size — the entry
//! carries the content `hash` it was fetched at (the manifest's per-master hash), so a
//! re-mastered kind reads as stale and is re-fetched + overwritten in place (no
//! accumulation). A hash MATCH is served with no network at all.
//!
//! Every op is best-effort: a private-mode / quota / unsupported failure degrades
//! to "no persistence" (the network fetch still runs), never a thrown error.

import type { TexMap } from "./urls";

const DB_NAME = "resonantdust-tex";
const STORE = "previews";
/** Bumped 1→2 when the key gained the `map` dimension; 2→3 for one-resolution (2026-08-02):
 *  the `size` dimension left the key — one row per (stem, map) at the manifest max. Old
 *  per-size rows become dead weight under the new `stem@map` keys — harmless (re-fetched once). */
const DB_VERSION = 3;

/** A persisted map: the raw PNG bytes for one map plus the content hash they were fetched
 *  at (`v`), for staleness checks. */
export interface CachedBytes {
  v: string;
  bytes: ArrayBuffer;
}

/** The composite store key for one (stem, map) — one-resolution: no size dimension. */
const mapKey = (stem: string, map: TexMap): string => `${stem}@${map}`;

let dbPromise: Promise<IDBDatabase> | null = null;

/** An open can HANG indefinitely — e.g. queued behind a pending `deleteDatabase` issued
 *  from devtools or another tab whose connection hasn't closed. Best-effort means that
 *  must degrade to "no persistence" (network still runs), not stall every texture load. */
const OPEN_TIMEOUT_MS = 1500;

function db(): Promise<IDBDatabase> {
  return (dbPromise ??= new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    const fail = (why: string) => {
      dbPromise = null; // retry on a later op — the wedge may have cleared by then
      // A post-timeout success would leak a connection that blocks future deletes/upgrades.
      req.onsuccess = () => req.result.close();
      reject(new Error(`previewCache: ${why}`));
    };
    const timer = setTimeout(() => fail("open timed out"), OPEN_TIMEOUT_MS);
    // Fires on first create AND on the 1→2 version bump — the store already exists on the
    // latter, so guard (a bare createObjectStore would throw ConstraintError). Old v1 rows
    // (keyed `stem@size`) are simply never read under the new `stem@size@map` keys.
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE)) req.result.createObjectStore(STORE);
    };
    req.onsuccess = () => {
      clearTimeout(timer);
      const d = req.result;
      // Another tab deleting/upgrading the DB must not wedge on OUR connection: close and
      // reopen lazily (a completed delete then just means a cold cache).
      d.onversionchange = () => {
        d.close();
        if (dbPromise) dbPromise = null;
      };
      resolve(d);
    };
    req.onerror = () => {
      clearTimeout(timer);
      fail(String(req.error));
    };
    req.onblocked = () => {
      clearTimeout(timer);
      fail("open blocked");
    };
  }));
}

/** The persisted bytes for `(stem, map)`, or null if absent / storage unavailable.
 *  The caller revalidates `.v` against the gate with a conditional request. */
export async function getCachedMap(stem: string, map: TexMap): Promise<CachedBytes | null> {
  try {
    const d = await db();
    return await new Promise<CachedBytes | null>((resolve, reject) => {
      const req = d.transaction(STORE, "readonly").objectStore(STORE).get(mapKey(stem, map));
      req.onsuccess = () => resolve((req.result as CachedBytes | undefined) ?? null);
      req.onerror = () => reject(req.error);
    });
  } catch {
    return null;
  }
}

/** Persist `entry` for `(stem, map)`, overwriting any prior version. Best-effort. */
export async function putCachedMap(stem: string, map: TexMap, entry: CachedBytes): Promise<void> {
  try {
    const d = await db();
    await new Promise<void>((resolve, reject) => {
      const tx = d.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(entry, mapKey(stem, map));
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch {
    /* storage unavailable — skip; the network fetch re-fetches next time */
  }
}

/** Ask the browser to make this origin's storage (IndexedDB included) exempt from
 *  automatic eviction. Best-effort: may be denied by browser heuristics, in which
 *  case IndexedDB is still durable under normal conditions. Call once at boot. */
export async function persistStorage(): Promise<boolean> {
  try {
    return (await navigator.storage?.persist?.()) ?? false;
  } catch {
    return false;
  }
}
