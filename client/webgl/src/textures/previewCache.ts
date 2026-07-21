//! Persistent LOD store (IndexedDB) — pins low-res texture bytes in a store the
//! browser's HTTP cache can't evict, so a returning player's placeholders decode
//! with ZERO network (and never get pushed out by large full-res masters, which
//! share the HTTP cache's one LRU pool).
//!
//! Keyed by `stem@size`, one entry per (kind, LOD) — the entry carries the content
//! `hash` it was fetched at (the manifest's per-master hash), so a re-mastered kind
//! reads as stale and is re-fetched + overwritten in place (no accumulation). A hash
//! MATCH is served with no network at all — the URL is content-addressed.
//! The largest masters are best left to the evictable HTTP cache (a lower LOD floor
//! catches an eviction gracefully), so callers persist the small buckets here.
//!
//! Every op is best-effort: a private-mode / quota / unsupported failure degrades
//! to "no persistence" (the network fetch still runs), never a thrown error. One PNG
//! per (stem, size, MAP) entry — the multi-channel cache (albedo|normal|depth|…).

import type { TexMap } from "./lod";

const DB_NAME = "resonantdust-tex";
const STORE = "previews";
/** Bumped 1→2 when the key gained the `map` dimension: old albedo-keyed rows (`stem@size`)
 *  become dead weight under the new `stem@size@map` keys — harmless (re-fetched once). */
const DB_VERSION = 2;

/** A persisted LOD: the raw PNG bytes for one map plus the content hash they were fetched
 *  at (`v`), for staleness checks. */
export interface LodBytes {
  v: string;
  bytes: ArrayBuffer;
}

/** The composite store key for one (stem, LOD size, map). */
const lodKey = (stem: string, size: number, map: TexMap): string => `${stem}@${size}@${map}`;

let dbPromise: Promise<IDBDatabase> | null = null;

function db(): Promise<IDBDatabase> {
  return (dbPromise ??= new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    // Fires on first create AND on the 1→2 version bump — the store already exists on the
    // latter, so guard (a bare createObjectStore would throw ConstraintError). Old v1 rows
    // (keyed `stem@size`) are simply never read under the new `stem@size@map` keys.
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE)) req.result.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  }));
}

/** The persisted LOD for `(stem, size, map)`, or null if absent / storage unavailable.
 *  The caller revalidates `.v` against the gate with a conditional request. */
export async function getLod(stem: string, size: number, map: TexMap): Promise<LodBytes | null> {
  try {
    const d = await db();
    return await new Promise<LodBytes | null>((resolve, reject) => {
      const req = d.transaction(STORE, "readonly").objectStore(STORE).get(lodKey(stem, size, map));
      req.onsuccess = () => resolve((req.result as LodBytes | undefined) ?? null);
      req.onerror = () => reject(req.error);
    });
  } catch {
    return null;
  }
}

/** Persist `entry` for `(stem, size, map)`, overwriting any prior version. Best-effort. */
export async function putLod(stem: string, size: number, map: TexMap, entry: LodBytes): Promise<void> {
  try {
    const d = await db();
    await new Promise<void>((resolve, reject) => {
      const tx = d.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(entry, lodKey(stem, size, map));
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
