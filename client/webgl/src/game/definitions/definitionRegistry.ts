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

/** What an id means: its taxonomy and which revision of it. */
export interface Definition {
  id: number;
  version: number;
  type: string;
  subType: string;
  kind: string;
  variant: string;
}

/** The tuple key — the four taxonomy names that identify a definition. */
function key(type: string, subType: string, kind: string, variant: string): string {
  return `${type}/${subType}/${kind}/${variant}`;
}

export class DefinitionRegistry {
  /** tuple key → the highest-version row seen for it. */
  private readonly byTuple = new Map<string, { id: number; version: number }>();
  /** id → what it MEANS. The reverse direction, which is what makes a stored id self-describing:
   *  a client receives a `definition_reference` off the wire and can say which definition and which
   *  REVISION it is, without asking the server. Every row is here, not just the newest — an old id
   *  is precisely the one whose meaning you need to look up. */
  private readonly byId = new Map<number, Definition>();
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

  /** What a `definition_reference` MEANS — its taxonomy and revision — or `null` if the registry
   *  has never seen it. The reverse of {@link resolve}.
   *
   *  This is the lookup a version-aware action needs ([version-predicates]): given an id off the
   *  wire, which revision is this, and does my implementation understand it? */
  lookup(id: number): Definition | null {
    return this.byId.get(id >>> 0) ?? null;
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
    const byId = new Map<number, Definition>();
    for (const [id, version, type, subType, kind, variant] of rows) {
      const k = key(type, subType, kind, variant);
      const cur = next.get(k);
      // F6: highest version wins for NEW placements. Older rows stay — they are what stored ids
      // mean — but they never win a name lookup. EVERY row goes into `byId`, for exactly that
      // reason: the reverse lookup exists to explain the old ones.
      if (!cur || version > cur.version) next.set(k, { id, version });
      byId.set(id >>> 0, { id: id >>> 0, version, type, subType, kind, variant });
    }
    this.byTuple.clear();
    for (const [k, v] of next) this.byTuple.set(k, v);
    this.byId.clear();
    for (const [k, v] of byId) this.byId.set(k, v);
    for (const fn of this.listeners) fn();
  }

  /** The three parallel arrays the wasm `Content.withRegistry` takes, built by pairing each
   *  registry row with the corpus def carrying the same taxonomy.
   *
   *  Keyed by NAME because that is what callers ask with ([F14]): `name → tuple` is authored in
   *  the corpus, `tuple → id` is this table, and binding them here collapses the two hops into
   *  the one lookup `tile_def_id` already performs. A def with no taxonomy contributes nothing
   *  and keeps resolving through its authored id. */
  bindTo(content: {
    tileNames(): string[];
    thingNames(): string[];
    tileTaxonomy(defId: number): string[] | undefined;
    thingTaxonomy(objectId: number): string[] | undefined;
    withRegistry(isTile: Uint8Array, names: string[], defs: Uint32Array): void;
  }): number {
    const isTile: number[] = [];
    const names: string[] = [];
    const defs: number[] = [];
    const bind = (tile: boolean, list: string[], tax: (i: number) => string[] | undefined) => {
      list.forEach((name, i) => {
        if (!name) return; // a retired id — a hole stays a hole
        const t = tax(i + 1);
        if (!t || t.length < 4) return;
        const id = this.resolve(t[0], t[1], t[2], t[3]);
        if (id === null) return;
        isTile.push(tile ? 1 : 0);
        names.push(name);
        // The FULL packed def. The kind half is what `tileDefId` serves; the type/subtype halves
        // are the taxonomy's numbering too, and dropping them here is what used to force a caller
        // to recover a pawn's species from its texture path.
        defs.push(id >>> 0);
      });
    };
    bind(true, content.tileNames(), (i) => content.tileTaxonomy(i));
    bind(false, content.thingNames(), (i) => content.thingTaxonomy(i));
    content.withRegistry(Uint8Array.from(isTile), names, Uint32Array.from(defs));
    return names.length;
  }
}

/** The process-wide registry — one numbering per client, like one corpus. */
export const definitionRegistry = new DefinitionRegistry();
