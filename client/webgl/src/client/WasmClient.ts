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

import { WorldClient, ticHz, defaultTicsPerTile } from "./wasm";

export { ticHz, defaultTicsPerTile };
import { assetBaseFromServerUrl } from "./environments";

/** Shared render delay `D`, in ms. Every client renders the world as of
 *  `syncedNow − RENDER_DELAY_MS`; using the SAME value on every client is what
 *  makes them agree on the instant they display. Sized above typical one-way
 *  latency + jitter + the server's write cadence — 300ms is imperceptible for a
 *  colony sim and doubles as the command-latency budget (a local action becomes
 *  visible this long after it's issued). */
export const RENDER_DELAY_MS = 300;

/** Clock discipline — how the smooth **Sync.now** clock chases the jittery **raw**
 *  server-offset estimate. Small errors are *slewed* (the clock ticks slightly
 *  fast/slow until it catches up) so motion never hitches and clients converge
 *  together; large errors (first sync, resync after a backgrounded tab) are
 *  *stepped* instantly because slewing multi-second gaps would keep the clock
 *  wrong for too long. The slew rate is well under 1000 ms/s, so Sync.now is
 *  always monotonic — it may tick slow, but never runs backward (except on a
 *  deliberate step). */
export const CLOCK_STEP_THRESHOLD_MS = 250;
export const CLOCK_SLEW_RATE_MS_PER_S = 50;

/** Which discipline regime the clock is in — surfaced to the debug HUD. */
export type SlewState = "step" | "slew" | "locked" | "—";

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
 *  the wire tag (`state`); `rows` counts the `Row` frames streamed down
 *  subscriptions and `rx` sums their serialized bytes. */
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

/** Clock-sync diagnostics for the debug HUD's "sync" tab. Fed by the wasm core's
 *  `clockSync` event — a login seed plus a ping/pong round-trip every couple of
 *  seconds — carrying the running offset/RTT estimate. */
export interface ClockStats {
  serverNowMs: number;
  synced: boolean;
  clientDelayMs: number;
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
  playerShardReference: number;
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

/** A cold zone's OBJECT row arrived (the object model): `typeReference` is the row's
 *  shared `object_type_reference` (type / subtype = biome / layer), `kinds` its members
 *  as `object_kind_reference`s (u32 each). A `biome-tile` row is the ground, a
 *  `biome-thing` row the scatter. Supersedes tiles/things; fires per cold row. */
export type ColdTilesHandler = (macroPosition: number, subtypeId: number, layerId: number, tic: number, tiles: Uint16Array) => void;
export type ColdThingsHandler = (macroPosition: number, subtypeId: number, layerId: number, tic: number, things: Uint32Array) => void;
/** A cold **overlay** row (a cold shard's `state`) — a per-cell mutation composited over the
 *  baseline at `positionReference`. `removed` clears it. Keyed by `entityReference`. `tic` orders it
 *  against the baseline row's `tic` (most recent wins — resolves the cold-row/overlay arrival race). */
export interface ColdStateOverride {
  macroPosition: number;
  entityReference: number;
  positionReference: number;
  definitionReference: number;
  data: number;
  tic: number;
  removed: boolean;
}
export type ColdStateHandler = (o: ColdStateOverride) => void;
/** A zone's subscription closed — drop its entities. */
export type ZoneClosedHandler = (macroPosition: number) => void;

/** A composed entity from `data_shard.state`. `entityReference` (`server_reference:8 |
 *  object_reference:24`) is JS-safe (a `u32`); its top nibble is the object type. The host keys
 *  the mover by it and draws `definitionReference` (the content kind → sprite) at global
 *  `(tileX, tileY)` facing `facing`, interpolating position by `tic`. */
export interface StateObject {
  macroPosition: number;
  entityReference: number;
  /** The entity's content kind → sprite (`0` until a definition verb plumbs it). */
  definitionReference: number;
  /** Global tile coordinates. */
  tileX: number;
  tileY: number;
  /** Facing: 0=south, 1=east, 2=north, 3=west. */
  facing: number;
  tic: number;
  removed: boolean;
}
/** A state object changed — upsert or drop it. */
export type StateObjectHandler = (obj: StateObject) => void;

/** A pawn's decoded part slots (human-pawns P0) — from the payload sidecar's `PART` entries:
 *  each `{slot, def}` names the FULL definition the part slot draws (body = slot 0, head =
 *  slot 1). Joined to the entity's {@link StateObject}s by the host — either may arrive first. */
export interface PawnParts {
  macroPosition: number;
  entityReference: number;
  tic: number;
  parts: { slot: number; def: number }[];
  /** The RAW payload opcode stream (needs-moodlets P4) — fed verbatim to the wasm
   *  `pawnMoodlets`/`pawnMood` eval; the host never decodes NEED/MOODLET entries itself. */
  payload: Uint32Array;
}
export type PawnPartsHandler = (p: PawnParts) => void;

/** A promoted movement INTENT (`ACTIONS.md` §Movement): `entityReference` is heading to global
 *  tile `(tileX, tileY)`, its first hop composed at `eventTic`. Clients SPECULATE position from
 *  this — per-hop state never fans out. */
export interface MoveIntent {
  macroPosition: number;
  entityReference: number;
  tileX: number;
  tileY: number;
  eventTic: number;
}

export type MoveIntentHandler = (intent: MoveIntent) => void;

const NOOP_UNSUB = (): void => { /* nothing subscribed */ };

/** How long to wait for login to complete (gateway resolve + connect + auth)
 *  before giving up, so a stalled handshake can't hang the login form forever. */
const LOGIN_TIMEOUT_MS = 12_000;

/** A tagged event from the wasm `WorldClient` (mirror of the Rust `Event` enum,
 *  marshaled in `shared/wasm`'s `event_to_js`). */
type WorldEvent =
  | { kind: "loginStarted"; name: string }
  | { kind: "serverResolved"; serverId: number; url: string; reused: boolean }
  | { kind: "loggedIn"; playerId: number; playerShardReference: number; serverUrl: string }
  | { kind: "loginFailed"; reason: string }
  | { kind: "disconnected"; reason: string | null }
  | { kind: "status"; message: string }
  | { kind: "coldTiles"; macroPosition: number; subtypeId: number; layerId: number; tic: number; tiles: Uint16Array }
  | { kind: "coldThings"; macroPosition: number; subtypeId: number; layerId: number; tic: number; things: Uint32Array }
  | {
      kind: "coldState";
      macroPosition: number;
      entityReference: number;
      positionReference: number;
      definitionReference: number;
      data: number;
      tic: number;
      removed: boolean;
    }
  | {
      kind: "stateObject";
      macroPosition: number;
      entityReference: number;
      definitionReference: number;
      tileX: number;
      tileY: number;
      facing: number;
      tic: number;
      removed: boolean;
    }
  | {
      kind: "pawnParts";
      macroPosition: number;
      entityReference: number;
      tic: number;
      /** (slot, def) pairs flattened `[slot, def, slot, def, …]`. */
      parts: Uint32Array;
      /** The raw payload opcode stream (needs-moodlets P4). */
      payload: Uint32Array;
    }
  | { kind: "zoneClosed"; macroPosition: number }
  | { kind: "paused"; paused: boolean }
  | { kind: "ticAnchor"; tic: number; wallMs: number; ticsPerSec: number }
  | {
      kind: "moveIntent";
      macroPosition: number;
      entityReference: number;
      tileX: number;
      tileY: number;
      eventTic: number;
    }
  | { kind: "callStats"; stats: CallStat[] }
  | { kind: "subStats"; open: number; total: number; tables: SubStat[] }
  | {
      kind: "clockSync";
      serverNowMs: number;
      synced: boolean;
      offsetMs: number;
      captures: number;
      rttSamples: number;
      rttMs: number | null;
      bestRttMs: number | null;
      bestOffsetMs: number | null;
      worstOffsetMs: number | null;
    };

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
  /** This session's `player_id`, set on `loggedIn` (`0` before). Derives the controllable pawn. */
  private playerId = 0;
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
  /** Raw offset (`serverNow − Date.now()`) from the core's windowed best-RTT
   *  estimate, refreshed on every synced `clockSync`. Jittery / steps per sample;
   *  it's the *target* the disciplined clock chases, and drives the HUD's raw
   *  **Server.now**. `0` until the first synced sample. */
  private rawOffsetMs = 0;
  /** Disciplined offset — the smoothly-slewed value {@link syncedNowMs} actually
   *  uses, chasing {@link rawOffsetMs} via step/slew. */
  private disciplinedOffsetMs = 0;
  /** Whether the disciplined offset has been primed from a first raw estimate. */
  private disciplineInit = false;
  /** `Date.now()` at the last discipline advance, for the slew's time step. */
  private lastDisciplineMs = 0;
  /** The regime the last discipline advance took — for the HUD. */
  private slewState: SlewState = "—";
  /** Whether the clock has at least one estimate (login seed or a pong). */
  private clockSynced = false;
  private readonly loggedInCbs = new Set<(serverUrl: string) => void>();
  private readonly coldTilesCbs = new Set<ColdTilesHandler>();
  private readonly coldThingsCbs = new Set<ColdThingsHandler>();
  private readonly coldStateCbs = new Set<ColdStateHandler>();
  private readonly zoneClosedCbs = new Set<ZoneClosedHandler>();
  private readonly stateObjectCbs = new Set<StateObjectHandler>();
  private readonly pawnPartsCbs = new Set<PawnPartsHandler>();
  private readonly pausedCbs = new Set<(paused: boolean) => void>();
  private readonly callStatCbs = new Set<(stats: CallStat[]) => void>();
  private readonly subStatCbs = new Set<(snap: SubStatsSnapshot) => void>();
  private readonly moveIntentCbs = new Set<MoveIntentHandler>();
  /** The wall↔tic anchor (first-pawns P3) — the freshest wire tic and the wall time it arrived.
   *  Fractional deltas extrapolate by {@link ticHz}; null until any state/event arrives. */
  private ticAnchor: { tic: number; wallMs: number; ticsPerSec: number } | null = null;

  constructor(private gatewayUrl: string) {
    // Debug hook, same convention as `__viewport` — lets the console probe ticDelta/anchors.
    (globalThis as unknown as { __client: WasmClient }).__client = this;
  }

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

  /** **Sync.now** — the disciplined server-clock estimate for *now*: the smooth,
   *  monotonic clock the render loop drives off (`syncedNowMs() − RENDER_DELAY_MS`
   *  picks the shared display instant). It slews toward the raw estimate rather
   *  than snapping, so it converges across clients without hitching. Advancing the
   *  discipline here means any caller (render loop, HUD) keeps it fresh. Before the
   *  first sample this is just `Date.now()` (offset 0) — rendering still works,
   *  just un-synced across clients. */
  syncedNowMs(): number {
    const now = Date.now();
    this.advanceDiscipline(now);
    return now + this.disciplinedOffsetMs;
  }

  /** **Server.now** — the *raw* server-clock estimate for now (`Date.now()` +
   *  windowed best-RTT offset). Jittery and step-prone; it's the target Sync.now
   *  chases. The debug HUD shows it alongside Sync.now to make convergence
   *  visible. */
  rawServerNowMs(): number {
    return Date.now() + this.rawOffsetMs;
  }

  /** Live clock-discipline readout for the debug HUD: the disciplined vs raw
   *  offsets and which regime the last advance took. */
  clockDiag(): { disciplinedOffsetMs: number; rawOffsetMs: number; slew: SlewState } {
    return {
      disciplinedOffsetMs: this.disciplinedOffsetMs,
      rawOffsetMs: this.rawOffsetMs,
      slew: this.clockSynced ? this.slewState : "—",
    };
  }

  /** Advance the disciplined offset toward the raw estimate. Steps for large
   *  errors (first sync / resync), slews for small ones at a bounded rate that
   *  keeps Sync.now monotonic. Idempotent within a frame — called back-to-back
   *  with `dt ≈ 0` it barely moves. */
  private advanceDiscipline(now: number): void {
    if (!this.clockSynced) return;
    const target = this.rawOffsetMs;
    if (!this.disciplineInit) {
      this.disciplinedOffsetMs = target;
      this.lastDisciplineMs = now;
      this.disciplineInit = true;
      this.slewState = "step";
      return;
    }
    const err = target - this.disciplinedOffsetMs;
    if (Math.abs(err) > CLOCK_STEP_THRESHOLD_MS) {
      this.disciplinedOffsetMs = target;
      this.slewState = "step";
    } else if (err === 0) {
      this.slewState = "locked";
    } else {
      const dt = Math.max(0, now - this.lastDisciplineMs);
      const maxStep = (CLOCK_SLEW_RATE_MS_PER_S * dt) / 1000;
      this.disciplinedOffsetMs += Math.max(-maxStep, Math.min(maxStep, err));
      this.slewState = "slew";
    }
    this.lastDisciplineMs = now;
  }

  /** Whether the clock offset has been estimated at least once. */
  clockIsSynced(): boolean {
    return this.clockSynced;
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
   * `player_shard_reference` on `loggedIn`, rejects (with a human-readable reason) on
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

  /** Add or move the viewport anchor `name` to global tile `(tileX, tileY)`, with the
   *  per-tier reach `radii` (in tiles). `soul` is `0` for a viewport. Idempotent — cheap to
   *  call as the view pans. No-op before login. (The world is a single 2D tile plane; the old
   *  `surface`/z-axis is retired.) */
  setAnchor(
    name: string,
    tileX: number,
    tileY: number,
    radii: AnchorRadii,
    soul: number,
  ): void {
    this.world?.setAnchor(
      name,
      tileX,
      tileY,
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

  /** Queue a raw action program (`docs/ACTIONS.md`) into the simulation. No-op before login. */
  queue(actions: Uint32Array): void {
    this.world?.queue(actions);
  }

  /** Move `entity` (an `entity_reference`) toward global tile `(tileX, tileY)`. No-op before login. */
  moveEntity(entity: number, tileX: number, tileY: number): void {
    this.world?.moveEntity(entity, tileX, tileY);
  }

  /** build-walls D5: order walls on the `(start..end)` tile rect's perimeter. */
  buildWall(startX: number, startY: number, endX: number, endY: number, object: number): void {
    this.world?.buildWall(startX, startY, endX, endY, object);
  }

  /** Place + promote `entity` at global tile `(tileX, tileY)`. No-op before login. */
  place(entity: number, tileX: number, tileY: number): void {
    this.world?.place(entity, tileX, tileY);
  }

  /** This session's controllable pawn `entity_reference` — a `TYPE_PAWN` (server byte `0x30`)
   *  reference whose object part is the `player_id`, so it never collides with an npc's wolves
   *  (objects `1..n`). `0` before login (no session). */
  private selfEntity(): number {
    return this.playerId ? 0x30_00_00_00 + (this.playerId & 0xff_ffff) : 0;
  }

  /** Move the session's own pawn toward global tile `(tileX, tileY)` (left-click). No-op if not
   *  logged in or the pawn hasn't been placed yet. */
  moveSelf(tileX: number, tileY: number): void {
    const e = this.selfEntity();
    if (e) this.moveEntity(e, tileX, tileY);
  }

  /** Place + promote the session's own pawn at global tile `(tileX, tileY)` (right-click) — the
   *  spawn/relocate of the controllable marker. No-op if not logged in. */
  placeSelf(tileX: number, tileY: number): void {
    const e = this.selfEntity();
    if (e) this.place(e, tileX, tileY);
  }

  /** Subscribe to simulation freeze changes (debug `/pause`). Fires with the new paused
   *  state whenever the shard's flag flips (and once on subscribe). Returns an unsubscribe. */
  onPaused(cb: (paused: boolean) => void): () => void {
    this.pausedCbs.add(cb);
    return () => this.pausedCbs.delete(cb);
  }

  /** Subscribe to a zone's cold **ground** (dense `kind_reference`s). Returns an unsubscribe. */
  onColdTiles(cb: ColdTilesHandler): () => void {
    this.coldTilesCbs.add(cb);
    return () => this.coldTilesCbs.delete(cb);
  }

  /** Subscribe to a zone's cold **scatter** (sparse `kind_pos_reference`s). Returns an unsubscribe. */
  onColdThings(cb: ColdThingsHandler): () => void {
    this.coldThingsCbs.add(cb);
    return () => this.coldThingsCbs.delete(cb);
  }

  /** Subscribe to cold **overlay** rows (per-cell mutations to composite over the baseline). Unsub. */
  onColdState(cb: ColdStateHandler): () => void {
    this.coldStateCbs.add(cb);
    return () => this.coldStateCbs.delete(cb);
  }

  /** Subscribe to zone-close notifications (a sub dropped). Returns an unsub. */
  onZoneClosed(cb: ZoneClosedHandler): () => void {
    this.zoneClosedCbs.add(cb);
    return () => this.zoneClosedCbs.delete(cb);
  }

  /** Subscribe to tick-pipeline entity changes (object-shard `state`). Returns an
   *  unsubscribe. */
  onStateObject(cb: StateObjectHandler): () => void {
    this.stateObjectCbs.add(cb);
    return () => this.stateObjectCbs.delete(cb);
  }

  /** Subscribe to pawn part-slot changes (the payload sidecar's decoded `PART` entries).
   *  Returns an unsubscribe. */
  onPawnParts(cb: PawnPartsHandler): () => void {
    this.pawnPartsCbs.add(cb);
    return () => this.pawnPartsCbs.delete(cb);
  }

  /** Subscribe to promoted movement INTENTS (`ACTIONS.md` §Movement — the channel speculation
   *  walks on; per-hop state never fans out). Returns an unsubscribe. */
  onMoveIntent(cb: MoveIntentHandler): () => void {
    this.moveIntentCbs.add(cb);
    return () => this.moveIntentCbs.delete(cb);
  }

  /** Fractional tics elapsed NOW since wire tic `t` (serial — correct across the u16 wrap;
   *  negative = `t` is still in the estimated future). `null` until any state/event has
   *  anchored the estimate. The speculation clock (first-pawns P3). */
  /** The LEARNED tic rate (tics/second) — the authored `ticHz` until an anchor refines it.
   *  The render-chase cap derives from this so "+20%" means 20% over TRUE speed. */
  ticsPerSec(): number {
    return this.ticAnchor?.ticsPerSec ?? ticHz();
  }

  ticDelta(t: number): number | null {
    if (!this.ticAnchor) return null;
    const serial = (((this.ticAnchor.tic - t) & 0xffff) << 16) >> 16; // sign-extend i16
    // Same timebase as the engine's anchor stamp (`js_sys::Date::now`), NOT performance.now().
    // Extrapolate at the LEARNED rate the anchor carries (pawn-movement F6) — the true tic
    // rate measurably drifts from the authored `ticHz`.
    return serial + ((Date.now() - this.ticAnchor.wallMs) / 1000) * this.ticAnchor.ticsPerSec;
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

  /** Clock-sync snapshots → debug HUD. Fires on every `clockSync` (login seed +
   *  each ping/pong). Only one consumer is kept (the HUD projector). */
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
      // Seed the estimator's rate from the last session's learned value (movement-hardening
      // F5 — kills the ~60 s cold-page warmup where first-trip landings correct by ~1 tile).
      // A clamped HINT: the engine ignores it once the stream anchors.
      try {
        const hint = Number(localStorage.getItem(`rd-tic-rate:${this.gatewayUrl}`));
        if (hint > 0) this.world.seedTicRate(hint);
      } catch { /* storage unavailable (private mode) — the warmup just runs */ }
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
        this.playerId = ev.playerId;
        this.pending?.resolve({
          playerId: ev.playerId,
          playerShardReference: ev.playerShardReference,
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
      case "coldTiles":
        for (const cb of this.coldTilesCbs) cb(ev.macroPosition, ev.subtypeId, ev.layerId, ev.tic, ev.tiles);
        break;
      case "coldThings":
        for (const cb of this.coldThingsCbs) cb(ev.macroPosition, ev.subtypeId, ev.layerId, ev.tic, ev.things);
        break;
      case "coldState":
        for (const cb of this.coldStateCbs)
          cb({
            macroPosition: ev.macroPosition,
            entityReference: ev.entityReference,
            positionReference: ev.positionReference,
            definitionReference: ev.definitionReference,
            data: ev.data,
            tic: ev.tic,
            removed: ev.removed,
          });
        break;
      case "stateObject":
        for (const cb of this.stateObjectCbs) {
          cb({
            macroPosition: ev.macroPosition,
            entityReference: ev.entityReference,
            definitionReference: ev.definitionReference,
            tileX: ev.tileX,
            tileY: ev.tileY,
            facing: ev.facing,
            tic: ev.tic,
            removed: ev.removed,
          });
        }
        break;
      case "pawnParts": {
        // Unflatten the (slot, def) pairs the wasm boundary shipped as a Uint32Array.
        const parts: { slot: number; def: number }[] = [];
        for (let i = 0; i + 1 < ev.parts.length; i += 2) {
          parts.push({ slot: ev.parts[i], def: ev.parts[i + 1] });
        }
        for (const cb of this.pawnPartsCbs) {
          cb({
            macroPosition: ev.macroPosition,
            entityReference: ev.entityReference,
            tic: ev.tic,
            parts,
            payload: ev.payload ?? new Uint32Array(0),
          });
        }
        break;
      }
      case "clockSync":
        // Refresh the *raw* offset target (the disciplined clock chases it in
        // `syncedNowMs`), then project the snapshot into the HUD's `ClockStats`.
        // `synced` gates the write so a pre-sync seed of 0 can't skew the clock.
        this.clockSynced = ev.synced;
        if (ev.synced) this.rawOffsetMs = ev.serverNowMs - Date.now();
        this.clockCb?.({
          serverNowMs: ev.serverNowMs,
          synced: ev.synced,
          clientDelayMs: RENDER_DELAY_MS,
          captures: ev.captures,
          bestOffsetMs: ev.bestOffsetMs,
          worstOffsetMs: ev.worstOffsetMs,
          rttMs: ev.rttMs,
          bestRttMs: ev.bestRttMs,
          rttSamples: ev.rttSamples,
        });
        break;
      case "zoneClosed":
        for (const cb of this.zoneClosedCbs) cb(ev.macroPosition);
        break;
      case "paused":
        for (const cb of this.pausedCbs) cb(ev.paused);
        break;
      case "ticAnchor":
        this.ticAnchor = { tic: ev.tic, wallMs: ev.wallMs, ticsPerSec: ev.ticsPerSec };
        // Persist the learned rate for the next page load's seed (F5).
        try {
          localStorage.setItem(`rd-tic-rate:${this.gatewayUrl}`, String(ev.ticsPerSec));
        } catch { /* storage unavailable — fine */ }
        break;
      case "moveIntent":
        for (const cb of this.moveIntentCbs) {
          cb({
            macroPosition: ev.macroPosition,
            entityReference: ev.entityReference,
            tileX: ev.tileX,
            tileY: ev.tileY,
            eventTic: ev.eventTic,
          });
        }
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
