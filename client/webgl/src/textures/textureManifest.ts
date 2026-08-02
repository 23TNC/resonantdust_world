//! The client mirror of the server's texture manifest — the authoritative index of
//! what is served, so the resolver builds every texture URL from it and never
//! speculatively requests one that isn't there (a 404 against R2 is a billable op).
//!
//! Per stem: a content `hash` (the URL's cache-buster — a re-master changes it, so a
//! stale URL 404s and we refetch here) and the master's `maxSize` — one-resolution F2:
//! THE size the stem packs at. Fetched at login ({@link setRoot}) and polled for
//! changes; a change fires {@link onChange} so the resolver drops stale packs and
//! re-fetches with the fresh hash.

import { manifestUrl, manifestVersionUrl } from "./urls";
import type { TexMap } from "./urls";

/** One stem's manifest row: the master's content hash, its short-axis ceiling, and which
 *  maps (albedo|normal|depth|emissive) actually have a leaf on disk — the resolver skips
 *  requesting a map that's absent (no speculative 404). `albedo` is always present. */
export interface ManifestEntry {
  hash: string;
  maxSize: number;
  maps: TexMap[];
  /** A LINKED (autotile) atlas: the master holds a `[cols, rows]` cell grid the client
   *  samples by UV rather than fetching per-cell stems. Absent for an ordinary texture. */
  grid?: [number, number];
  /** Normalized per-cell inset `[padU, padV]` (a fraction of the whole atlas) trimmed off
   *  each cell's UV rect so a downscale can't bleed a neighbour cell across a cell edge.
   *  Present iff {@link grid} is. */
  pad?: [number, number];
  /** The frame's WORLD SPAN in tiles (from the leaf's meta.json, authored in the DSL) —
   *  lighting-correctness P1b: the record layer's span source (never atlas px). */
  span?: number;
}

interface ManifestPayload {
  version: string;
  textures: Record<
    string,
    { hash: string; maxSize: number; lods: number[]; maps?: string[]; grid?: number[]; pad?: number[]; span?: number }
  >;
}

export class TextureManifest {
  /** The `/textures` root the manifest endpoints hang off. Empty until login. */
  private root = "";
  private version = "";
  private readonly entries = new Map<string, ManifestEntry>();
  private pollTimer: number | null = null;
  private readonly listeners = new Set<() => void>();

  /** Point at the world server's `/textures` root (at login) and fetch + poll the
   *  manifest. A repoint (new env) refetches from scratch. */
  setRoot(texturesRoot: string): void {
    this.root = texturesRoot;
    this.version = "";
    this.entries.clear();
    void this.fetchManifest();
    this.startPolling();
  }

  /** The manifest row for `stem`, or undefined when the stem has no master (the
   *  resolver then stays on geo — no speculative request). */
  entry(stem: string): ManifestEntry | undefined {
    return this.entries.get(stem);
  }

  /** Subscribe to "the manifest changed" (a re-master or newly-served stem).
   *  Returns an unsubscribe. */
  onChange(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  /** Force a refetch — the resolver calls this on a stale-hash 404 (a re-master the
   *  poll hasn't seen yet), so the retry uses the fresh hash. */
  refresh(): void {
    void this.fetchManifest();
  }

  /** Fetch + swap the manifest; fires {@link onChange} when the version moved. */
  private async fetchManifest(): Promise<void> {
    if (!this.root) return;
    try {
      const resp = await fetch(manifestUrl(this.root));
      if (!resp.ok) return;
      const payload = (await resp.json()) as ManifestPayload;
      if (payload.version === this.version) return;
      this.version = payload.version;
      this.entries.clear();
      for (const [stem, e] of Object.entries(payload.textures)) {
        // An older payload without `maps` predates the multi-map server → albedo-only.
        const maps = (e.maps as TexMap[] | undefined) ?? ["albedo"];
        // A linked-atlas stem carries `grid` + `pad`; an ordinary stem omits both.
        const grid =
          Array.isArray(e.grid) && e.grid.length === 2 ? ([e.grid[0], e.grid[1]] as [number, number]) : undefined;
        const pad =
          grid && Array.isArray(e.pad) && e.pad.length === 2 ? ([e.pad[0], e.pad[1]] as [number, number]) : undefined;
        this.entries.set(stem, { hash: e.hash, maxSize: e.maxSize, maps, grid, pad, span: e.span });
      }
      for (const fn of this.listeners) fn();
    } catch {
      /* server unreachable — retry next poll */
    }
  }

  /** Poll the cheap `/textures-manifest-version`; refetch the full manifest on a
   *  change. Idempotent — replaces any running poll. */
  private startPolling(intervalMs = 3000): void {
    this.stopPolling();
    this.pollTimer = window.setInterval(() => {
      if (!this.root) return;
      void (async () => {
        try {
          const resp = await fetch(manifestVersionUrl(this.root));
          if (!resp.ok) return;
          const version = (await resp.text()).trim();
          if (version && version !== this.version) await this.fetchManifest();
        } catch {
          /* unreachable — retry next tick */
        }
      })();
    }, intervalMs);
  }

  private stopPolling(): void {
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  /** Stop the poll (on teardown). */
  dispose(): void {
    this.stopPolling();
    this.listeners.clear();
  }
}
