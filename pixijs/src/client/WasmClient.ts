//! The main-thread bridge to the world server — the client core's facade.
//!
//! This wraps the Rust `client` crate compiled to wasm (`WorldClient` in the
//! `resonantdust-shared` bundle): login (the two-hop gateway → world-server
//! handshake) and the anchor-driven zone subscription engine. We send it the
//! same `Command` verbs the native headless driver does (`login`, `setAnchor`,
//! …) and it streams `Event`s back, which arrive here as small tagged objects
//! (`{ kind, … }`) on the constructor callback and fan out to typed listeners.
//!
//! The surface is kept close to the Rust `Command`/`Event` contract (`api.rs`).
//! The chat / call-stat / sub-stat hooks are still inert stubs the ported UI
//! reaches for; they return no-op unsubscribes until those subsystems return.

import { WorldClient } from "./wasm";
import { assetBaseFromServerUrl } from "./environments";

/** One chat message from the (future) chat feed. `sentAt` is a packed
 *  `[time_ms | seq]` key as a STRING — the u64 exceeds JS's safe-integer range,
 *  so it's carried as text (and is a stable per-message id / sort key). The
 *  current protocol has no chat frames, so this never fires yet. */
export interface ChatMessage {
  sentAt: string;
  senderPlayerId: number;
  senderName: string;
  body: string;
}

/** One command type's gateway-call tally for the debug HUD's "calls" tab.
 *  `command` is the wire tag (`login` / `sub_zone` / `unsub` / `release`);
 *  `requests` / `tx` count outbound frames + their bytes; `ok` / `err` / `rx` the
 *  correlated replies + their bytes (fire-and-forget frames have no reply). */
export interface CallStat {
  command: string;
  requests: number;
  ok: number;
  err: number;
  tx: number;
  rx: number;
}

/** One relayed-row table's data tally for the debug HUD's "subs" tab. `table` is
 *  the wire tag (`cold_zone` / `hot_tile` / `hot_thing` / `free_thing`); `rows`
 *  counts the `Row` frames streamed down subscriptions and `rx` sums their
 *  serialized bytes. */
export interface SubStat {
  table: string;
  rows: number;
  rx: number;
}

/** A subscription-data snapshot for the "subs" tab: the live `open` zone-sub gauge,
 *  the cumulative `total` ever issued, and the per-table byte breakdown. */
export interface SubStatsSnapshot {
  open: number;
  total: number;
  tables: SubStat[];
}

/** Clock-sync diagnostics for the debug HUD's "sync" tab. The wasm `LoggedIn`
 *  event carries no clock sample, so this is inert for now — a running sync
 *  lands with a later clock frame. */
export interface ClockStats {
  serverNowMs: number;
  synced: boolean;
  clientDelayMs: number;
  runningDelayMs: number;
  runningDeltaMs: number;
  deltaMs: number | null;
  captures: number;
  bestOffsetMs: number | null;
  worstOffsetMs: number | null;
  rttMs: number | null;
  bestRttMs: number | null;
  rttSamples: number;
}

/** What a successful {@link WasmClient.login} resolves to. Mirrors the Rust
 *  `Event::LoggedIn` payload plus the gateway's `reused` affinity flag. */
export interface LoginResult {
  playerId: number;
  dataShard: number;
  /** The world-server WS URL the gateway resolved and we're now connected to. */
  serverUrl: string;
  /** `true` when the gateway reused an existing session pin (reconnect
   *  affinity), `false` when it freshly allocated a server. */
  reused: boolean;
}

/** Coarse progress callback for the login UI — the scene narrates these into its
 *  status line as the handshake advances through its stages. */
export type LoginProgress = (message: string) => void;

/** Per-tier anchor reach, in tiles (active ⊆ hot ⊆ warm ⊆ cold). Only the
 *  `active` ring opens subscriptions; the rest are the hysteresis ladder. */
export interface AnchorRadii {
  active: number;
  hot: number;
  warm: number;
  cold: number;
}

/** A cold zone's tiles arrived: `tiles` is the `ZONE_TILES` packed slots. */
export type ZoneTilesHandler = (zoneId: number, tiles: Uint16Array) => void;
/** A cold zone's things arrived: `things` is its packed thing entries
 *  (`x:4 | y:4 | rotation:2 | object_id:12` each) — the worldgen-scattered flora.
 *  Fires alongside the tiles on every cold delivery; an empty array clears them. */
export type ZoneThingsHandler = (zoneId: number, things: Uint32Array) => void;
/** A zone's subscription closed — drop its sprites. */
export type ZoneClosedHandler = (zoneId: number) => void;

/** One loose thing changed in a subscribed zone (object-shard `free_things`).
 *  `removed` = true drops the sprite keyed by `objectId`; otherwise upsert one at
 *  `location` shifted by the sub-tile `offset`. Insert and update both arrive as
 *  `removed: false` (the host keys by `objectId`). */
export interface FreeThing {
  zoneId: number;
  objectId: number;
  removed: boolean;
  location: number;
  rotation: number;
  id: number;
  offset: number;
}
/** A loose thing changed — upsert or drop it. */
export type FreeThingHandler = (thing: FreeThing) => void;

const NOOP_UNSUB = (): void => { /* nothing subscribed */ };

/** How long to wait for login to complete (gateway resolve + connect + auth)
 *  before giving up, so a stalled handshake can't hang the login form forever. */
const LOGIN_TIMEOUT_MS = 12_000;

/** A tagged event from the wasm `WorldClient` (mirror of the Rust `Event` enum,
 *  marshaled in `shared/wasm`'s `event_to_js`). */
type WorldEvent =
  | { kind: "loginStarted"; name: string }
  | { kind: "serverResolved"; serverId: number; url: string; reused: boolean }
  | { kind: "loggedIn"; playerId: number; dataShard: number; serverUrl: string }
  | { kind: "loginFailed"; reason: string }
  | { kind: "disconnected"; reason: string | null }
  | { kind: "status"; message: string }
  | { kind: "zoneTiles"; zoneId: number; tiles: Uint16Array }
  | { kind: "zoneThings"; zoneId: number; things: Uint32Array }
  | {
      kind: "zoneFreeThing";
      zoneId: number;
      objectId: number;
      removed: boolean;
      location: number;
      rotation: number;
      id: number;
      offset: number;
    }
  | { kind: "zoneClosed"; zoneId: number }
  | { kind: "callStats"; stats: CallStat[] }
  | { kind: "subStats"; open: number; total: number; tables: SubStat[] };

/** An in-flight login awaiting its terminal event. */
interface PendingLogin {
  resolve: (result: LoginResult) => void;
  reject: (err: Error) => void;
  onProgress?: LoginProgress;
}

/**
 * Client bridge. Constructed with the **gateway** HTTP base it should ask for a
 * world server (`http(s)://host:port`, no trailing `/server`). The underlying
 * wasm client is created lazily on first {@link login} (after the wasm module is
 * initialised) and re-created if the gateway is repointed.
 */
export class WasmClient {
  /** The wasm world client, or null before the first login. */
  private world: WorldClient | null = null;
  /** The gateway the live `world` was constructed against, to detect repoints. */
  private worldGateway: string | null = null;
  /** The in-flight login, if any. */
  private pending: PendingLogin | null = null;
  /** `reused` from the most recent `serverResolved`, folded into `LoginResult`. */
  private lastReused = false;
  /** The world server's URL from the most recent successful login (its WS base) —
   *  the client derives the HTTP ASSET base from this (`/content` + `/textures`
   *  live on the server now). `null` until logged in / after disconnect. */
  private serverUrl: string | null = null;

  private clockCb: ((stats: ClockStats) => void) | null = null;
  private readonly loggedInCbs = new Set<(serverUrl: string) => void>();
  private readonly zoneTilesCbs = new Set<ZoneTilesHandler>();
  private readonly zoneThingsCbs = new Set<ZoneThingsHandler>();
  private readonly zoneClosedCbs = new Set<ZoneClosedHandler>();
  private readonly freeThingCbs = new Set<FreeThingHandler>();
  private readonly callStatCbs = new Set<(stats: CallStat[]) => void>();
  private readonly subStatCbs = new Set<(snap: SubStatsSnapshot) => void>();

  constructor(private gatewayUrl: string) {}

  /** Repoint the client at a different gateway HTTP base (the login screen's
   *  Server dropdown does this before each attempt). The live wasm client is
   *  re-created against the new base on the next {@link login}. */
  setGatewayUrl(url: string): void {
    this.gatewayUrl = url;
  }

  /** The gateway HTTP base the client currently points at (used for `GET /server`). */
  currentGatewayUrl(): string {
    return this.gatewayUrl;
  }

  /** The HTTP base of the world server the client is logged into — where the
   *  DSL corpus (`/content`) and textures (`/textures`) are fetched from — or `""`
   *  before login / after disconnect. Derived from the login-discovered WS URL
   *  (`ws://host:port/ws` → `http://host:port`). */
  assetBase(): string {
    return this.serverUrl ? assetBaseFromServerUrl(this.serverUrl) : "";
  }

  /** Subscribe to successful logins (fires with the world server's WS URL). The
   *  boot wiring uses this to repoint the texture resolver + pull the server's
   *  corpus once the server is known. Returns an unsubscribe. */
  onLoggedIn(cb: (serverUrl: string) => void): () => void {
    this.loggedInCbs.add(cb);
    return () => this.loggedInCbs.delete(cb);
  }

  /**
   * Log in as `name` (trust-on-first-use). Drives the wasm engine's gateway →
   * connect → authenticate sequence; resolves with the bound `player_id` /
   * `data_shard` on `loggedIn`, rejects (with a human-readable reason) on
   * `loginFailed` / disconnect / timeout. `onProgress` is narrated into the
   * login form's status line as each stage begins.
   */
  login(name: string, onProgress?: LoginProgress): Promise<LoginResult> {
    const world = this.ensureWorld();

    // A new login supersedes any in-flight one.
    if (this.pending) {
      this.pending.reject(new Error("login superseded"));
      this.pending = null;
    }

    return new Promise<LoginResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        if (this.pending) {
          this.pending = null;
          reject(new Error("login timed out"));
        }
      }, LOGIN_TIMEOUT_MS);

      this.pending = {
        resolve: (result) => {
          clearTimeout(timer);
          resolve(result);
        },
        reject: (err) => {
          clearTimeout(timer);
          reject(err);
        },
        onProgress,
      };

      onProgress?.("Asking the gateway for a world server…");
      world.login(name);
    });
  }

  /** Drop the world-server connection and clear the session (the Rust
   *  `Command::Logout`). Safe to call when not connected. */
  logout(): void {
    this.world?.logout();
  }

  /** Add or move the viewport anchor `name` to global tile `(tileX, tileY)` on
   *  `surface`, with the per-tier reach `radii` (in tiles). `soul` is `0` for a
   *  viewport. Idempotent — cheap to call as the view pans. No-op before login. */
  setAnchor(
    name: string,
    tileX: number,
    tileY: number,
    surface: number,
    radii: AnchorRadii,
    soul: number,
  ): void {
    this.world?.setAnchor(
      name,
      tileX,
      tileY,
      surface,
      radii.active,
      radii.hot,
      radii.warm,
      radii.cold,
      soul,
    );
  }

  /** Remove the anchor `name`, closing any subscriptions only it held. */
  removeAnchor(name: string): void {
    this.world?.removeAnchor(name);
  }

  /** Release the thing affixed at `(zoneId, location)` into the object shard —
   *  it vanishes from the world and reappears as a loose thing. The server drives
   *  the transfer; the result arrives on existing subscriptions. No-op before
   *  login. */
  release(zoneId: number, location: number): void {
    this.world?.release(zoneId, location);
  }

  /** Subscribe to cold-zone tile deliveries. Returns an unsubscribe. */
  onZoneTiles(cb: ZoneTilesHandler): () => void {
    this.zoneTilesCbs.add(cb);
    return () => this.zoneTilesCbs.delete(cb);
  }

  /** Subscribe to cold-zone thing deliveries (worldgen-scattered flora). Returns
   *  an unsubscribe. */
  onZoneThings(cb: ZoneThingsHandler): () => void {
    this.zoneThingsCbs.add(cb);
    return () => this.zoneThingsCbs.delete(cb);
  }

  /** Subscribe to zone-close notifications (a sub dropped). Returns an unsub. */
  onZoneClosed(cb: ZoneClosedHandler): () => void {
    this.zoneClosedCbs.add(cb);
    return () => this.zoneClosedCbs.delete(cb);
  }

  /** Subscribe to loose-thing changes (object-shard `free_things`). Returns an
   *  unsubscribe. */
  onFreeThing(cb: FreeThingHandler): () => void {
    this.freeThingCbs.add(cb);
    return () => this.freeThingCbs.delete(cb);
  }

  /** ChatPanel feed subscription. The protocol carries no chat frames yet, so
   *  this never fires — wired for when the feed returns. */
  onChat(_cb: (messages: ChatMessage[]) => void): () => void {
    return NOOP_UNSUB;
  }

  /** Send a chat message — dropped until the protocol carries chat frames. */
  sendChat(_body: string): void {
    /* no chat frames yet — drop */
  }

  /** Clock-sync snapshots → debug HUD. Inert for now (no clock frame yet). */
  onClockStats(cb: (stats: ClockStats) => void): () => void {
    this.clockCb = cb;
    return () => {
      if (this.clockCb === cb) this.clockCb = null;
    };
  }

  /** Per-command call tally → debug HUD "calls" tab. Fires with the full snapshot
   *  on every `callStats` event (after each outbound frame / correlated reply).
   *  Returns an unsubscribe. */
  onCallStats(cb: (stats: CallStat[]) => void): () => void {
    this.callStatCbs.add(cb);
    return () => this.callStatCbs.delete(cb);
  }

  /** Subscription-data tally → debug HUD "subs" tab. Fires with the full snapshot
   *  (open/total gauge + per-table byte breakdown) whenever a subscription
   *  opens/closes or a `Row` frame lands. Returns an unsubscribe. */
  onSubStats(cb: (snap: SubStatsSnapshot) => void): () => void {
    this.subStatCbs.add(cb);
    return () => this.subStatCbs.delete(cb);
  }

  /** Gate content hot-swap notification. Superseded — content hot-swap is driven
   *  by polling the gateway's `GET /content-version` in `contentBoot`
   *  (`startContentPolling` → `onContentReloaded`), not over this client's stream,
   *  since the gateway (not the world server) owns the corpus. Kept as a no-op for
   *  the ported UI that still references it. */
  onContentChanged(_cb: () => void): () => void {
    return NOOP_UNSUB;
  }

  // ── internals ───────────────────────────────────────────────────────

  /** Get the wasm world client, (re)creating it if absent or if the gateway was
   *  repointed since it was built. The event callback is permanent — it fans
   *  every engine event out to this bridge's listeners. */
  private ensureWorld(): WorldClient {
    if (!this.world || this.worldGateway !== this.gatewayUrl) {
      this.world?.shutdown();
      this.world = new WorldClient(this.gatewayUrl, (ev: WorldEvent) => this.onEvent(ev));
      this.worldGateway = this.gatewayUrl;
    }
    return this.world;
  }

  /** Route one tagged engine event: resolve/reject the in-flight login, narrate
   *  progress, or fan zone deliveries out to listeners. */
  private onEvent(ev: WorldEvent): void {
    switch (ev.kind) {
      case "loginStarted":
        break;
      case "serverResolved":
        this.lastReused = ev.reused;
        this.pending?.onProgress?.(`Connecting to ${ev.url}…`);
        break;
      case "loggedIn":
        // Record the world server so asset fetches (`/content` + `/textures`)
        // target it, then notify listeners (they repoint the texture resolver +
        // pull the server's corpus).
        this.serverUrl = ev.serverUrl;
        this.pending?.resolve({
          playerId: ev.playerId,
          dataShard: ev.dataShard,
          serverUrl: ev.serverUrl,
          reused: this.lastReused,
        });
        this.pending = null;
        for (const cb of this.loggedInCbs) cb(ev.serverUrl);
        break;
      case "loginFailed":
        this.pending?.reject(new Error(ev.reason));
        this.pending = null;
        break;
      case "disconnected":
        this.serverUrl = null;
        if (this.pending) {
          this.pending.reject(new Error(ev.reason ?? "disconnected"));
          this.pending = null;
        }
        break;
      case "status":
        this.pending?.onProgress?.(ev.message);
        break;
      case "zoneTiles":
        for (const cb of this.zoneTilesCbs) cb(ev.zoneId, ev.tiles);
        break;
      case "zoneThings":
        for (const cb of this.zoneThingsCbs) cb(ev.zoneId, ev.things);
        break;
      case "zoneFreeThing":
        for (const cb of this.freeThingCbs) {
          cb({
            zoneId: ev.zoneId,
            objectId: ev.objectId,
            removed: ev.removed,
            location: ev.location,
            rotation: ev.rotation,
            id: ev.id,
            offset: ev.offset,
          });
        }
        break;
      case "zoneClosed":
        for (const cb of this.zoneClosedCbs) cb(ev.zoneId);
        break;
      case "callStats":
        for (const cb of this.callStatCbs) cb(ev.stats);
        break;
      case "subStats":
        for (const cb of this.subStatCbs) {
          cb({ open: ev.open, total: ev.total, tables: ev.tables });
        }
        break;
    }
  }
}
