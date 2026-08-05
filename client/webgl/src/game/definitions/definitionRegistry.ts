//! The DEFINITION REGISTRY, client side (work `2026-08-04-definition-registry` P4).
//!
//! The corpus DESCRIBES; the server NUMBERS. This holds the numbering, fetched from the world
//! server's `GET /definitions`, so a client can resolve `taxonomy → definition_reference` **locally**
//! — no round-trip, and no names on the action wire ([F9]).
//!
//! **Resolution takes the highest version** ([F6]). That is the whole versioning policy in one
//! line: a NEW placement gets the newest definition, while an object already holding an older id
//! keeps it and keeps behaving as that definition describes. An old apple stays an old apple —
//! same weight, same expiry — until it is spent. There is no migration sweep, deliberately.
//!
//! **A stale registry resolves to an older version**, which is safe for exactly the same reason,
//! and self-corrects on the next fetch.
//!
//! This never answers questions about a STORED id. A stored id is already the answer — the render
//! path decodes it from its own bits and never consults a table ([I6]).

/** One registry row as the wire sends it: `[id, version, type, subType, kind, variant]`. */
type Row = [number, number, string, string, string, string];

interface RegistryPayload {
  definitions: Row[];
}

/** The tuple key — the four taxonomy names that identify a definition. */
function key(type: string, subType: string, kind: string, variant: string): string {
  return `${type}/${subType}/${kind}/${variant}`;
}

export class DefinitionRegistry {
  /** tuple key → the highest-version row seen for it. */
  private readonly byTuple = new Map<string, { id: number; version: number }>();
  /** Listeners fired after a successful swap — the mirror of `onContentReloaded`. */
  private readonly listeners = new Set<() => void>();

  /** How many definitions are known. `0` before the first successful fetch. */
  get size(): number {
    return this.byTuple.size;
  }

  /** The `definition_reference` for a taxonomy tuple, or `null` if the registry has never seen it
   *  (an unseeded registry, a stale one, or a genuinely unknown definition — the caller must not
   *  guess a number, which is the whole point of the registry owning them). */
  resolve(type: string, subType: string, kind: string, variant: string): number | null {
    return this.byTuple.get(key(type, subType, kind, variant))?.id ?? null;
  }

  /** Subscribe to "the registry changed". Returns an unsubscribe. */
  onChange(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Fetch + swap. Throws on a non-OK response; the caller keeps the previous table, which is the
   *  safe direction — an older numbering still resolves to definitions that exist. */
  async load(serverBase: string): Promise<void> {
    const resp = await fetch(`${serverBase}/definitions`);
    if (!resp.ok) throw new Error(`GET /definitions: ${resp.status}`);
    const payload = (await resp.json()) as RegistryPayload;
    this.swap(payload.definitions ?? []);
  }

  /** Replace the table from raw rows. Split out so a test can drive it without a server. */
  swap(rows: Row[]): void {
    const next = new Map<string, { id: number; version: number }>();
    for (const [id, version, type, subType, kind, variant] of rows) {
      const k = key(type, subType, kind, variant);
      const cur = next.get(k);
      // F6: highest version wins for NEW placements. Older rows stay in the payload — they are
      // what stored ids mean — but they never win a name lookup.
      if (!cur || version > cur.version) next.set(k, { id, version });
    }
    this.byTuple.clear();
    for (const [k, v] of next) this.byTuple.set(k, v);
    for (const fn of this.listeners) fn();
  }
}

/** The process-wide registry — one numbering per client, like one corpus. */
export const definitionRegistry = new DefinitionRegistry();
