//! The mover layer — the tick pipeline's mobile entities (pawns) fed into the viewport's WARM
//! cache as ordinary primitives.
//!
//! A pawn (a wolf) is a WARM prim: a bottom-anchored sprite whose texture stem comes from the
//! content bundle (`thing_texture_stems[objKind-1]`, e.g. `pawn/animal/wolf/default`), its facing
//! picked from the entity's `rotation` via {@link moverSlotTexture} (the SAME rotation→texture+flip
//! table cold things use), its box placed from the def layout via {@link placeThing} (the SAME
//! footprint/anchor/size resolution cold things use). It bakes through the warm
//! cache's albedo/normal/surface/zdepth-world channels EXACTLY like a cold thing — geo→real
//! streaming, material reconstruction, coverage keying, and lighting + shadow all handled by the
//! shared pipeline. This layer just SYNCS pawn state into warm prims; it owns no rendering.
//!
//! **Movement is SPECULATED** (first-pawns P3, `ACTIONS.md` §Movement): a promoted
//! {@link MoveIntent} announces `entity → dest` once; per-hop state never fans out. Each frame
//! ({@link tick}) the layer walks the pawn fractionally along the server's OWN stepping rule
//! (greedy straight line, e/w-first facing) at the pawn's DERIVED `ground_speed` in
//! tics-per-tile (input-rework F8 — `pawnGroundSpeed` over its fanned rows: the SAME
//! `stat_eval` the worker spaces hops with), driven by the client's wall↔tic estimate — so
//! the wolf GLIDES between tiles with zero per-hop bandwidth.
//! Authoritative `State` rows snap/reseed the speculation and log the observed error (F8 —
//! the data the re-anchor cadence will be tuned on); `ZoneClosed` drops it.

import type { Texture } from "../../gl";
import type { WasmClient, StateObject, MoveIntent } from "../../client/WasmClient";
import { defaultTicsPerTile } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import type { PrimitiveLight } from "../viewport/SquareCache";
import { SQUARE } from "../viewport/squareMath";
import type { PackedChannel } from "../viewport/material";
import { moverSlotTexture } from "./WorldBridge";
import { placeThing, readLayout } from "./thingPlacement";

/** Base zIndex for warm pawn sprites — above the ground (tiles are `0`), matching cold things
 *  ({@link WorldBridge}'s `THING_Z_BASE`); the pawn's anchor tile-row is added so overlapping
 *  pawns/things paint front-over-back. */
/** z-positioning F1: named `*_ORDER`, not `*_Z`. Since `unit.z` means HEIGHT, a second `z` meaning
 *  DRAW ORDER beside it is how someone later reads a painter's key as an elevation and spends a day
 *  on it. These are ordering keys and nothing else. */
const PAWN_ORDER_BASE = 1;

/** One part SLOT of a pawn kind's visual skeleton — the wasm `moverParts` row (human-pawns
 *  P2). Slot order = DSL `^prim call` order; slot 0 boxes the carrier; `part` names which
 *  `<part>` files of the resolved leaf the slot draws; `offset*` places the slot in tiles
 *  relative to slot 0's anchor.
 *
 *  `scale` is the slot's PRE-ATLAS sprite scale (pawn-part-placement F4), applied to the art
 *  inside its own `span × span` frame — NOT to the drawn box. It used to multiply slot 0's drawn
 *  size, which made the drawn box disagree with the def's pow2 frame span and cast a silhouette
 *  1/scale too big ([I6]). It is therefore ABSOLUTE now: a `span 1` slot at `scale 0.5` is half a
 *  tile of art, whatever slot 0 does. */
interface MoverPart {
  stem: string | null;
  part: number;
  scale: number;
  offsetX: number;
  /** Placement offset on the GROUND plane, in tiles — a part genuinely further north. */
  offsetY: number;
  /** HEIGHT off the ground, in tiles ({@link MoverPart.offsetY}'s opposite in meaning, its twin
   *  on screen). Drawn as a 1:1 northward shift, so it looks exactly like `offsetY` — but it
   *  travels to the record as an ELEVATION, so the shadow system keeps the carrier's ground
   *  footprint and body + head cast one aligned shadow (z-positioning P3, F5). */
  offsetZ: number;
  /** Draw-order offset along the view's depth axis, in the pawn's OWN frame: positive = toward
   *  the viewer when the pawn faces the camera. {@link facingDepth} negates it when the pawn
   *  faces away, so `+1` reads as "head over body, except from behind" (F1). */
  depth: number;
  size: number;
  span: number;
  anchorX: number;
  anchorY: number;
  spriteAnchorX: number;
  spriteAnchorY: number;
  tint: number;
  geoColor: number;
  /** The slot's authored SUBFRAME rects, flat `[variant][rotation] × [x,y,w,h,ax,ay]`
   *  (16 × 16 × 6 = 1536) — registered per resolved stem in {@link applyVisual}
   *  (human-pawns-redux P1, closing subframe-ingest I11: these were emitted by
   *  `moverParts` and never read, so every mover cropped by defaults). */
  subframes?: Float64Array;
}


/** The pawn's live WALK, as the client still needs to know about it — no longer a speculation.
 *  Core computes where the pawn is; this records only what the DISPLAY still asks: whether the
 *  pawn is walking (the intent strip and the arm-time facing aim), where to (arrival clears it),
 *  and the last point applied, which is what the divergence probe measures belief against.
 *
 *  `fromX/fromY`, `eventTic`, `ticsPerTile` and `path` are carried for the probe's bookkeeping
 *  only — nothing steps along them. They go when the probe moves out of this file. */
interface Walk {
  fromX: number;
  fromY: number;
  destX: number;
  destY: number;
  eventTic: number;
  ticsPerTile: number;
  appliedX: number;
  appliedY: number;
  path: { x: number; y: number }[] | null;
}

/** The render applies a new position only past this tile delta — keeps the warm re-bake cadence
 *  proportional to actual motion, not the frame rate. */
const RENDER_APPLY_EPS = 1 / 32;

/** How much of a z-ROW one unit of authored slot `depth` is worth (F1). Slot ordering must stay
 *  strictly INSIDE the pawn's row so the row keeps deciding which pawn is in front: with the
 *  per-slot tiebreak below, `|depth| < 45` is safely within `(-0.5, +0.5)`. */
const SLOT_DEPTH_ORDER = 0.01;
/** A stable within-depth tiebreak so two slots at the same depth keep their authored order. */
const SLOT_INDEX_ORDER = 0.0001;

/** The slot's depth in SCREEN terms: authored depth is in the pawn's own frame (positive = toward
 *  the viewer when it faces the camera), so a pawn facing AWAY (north, `rotation 2`) has its local
 *  depth axis pointing away and the sign flips. East/west put the axis across the view, where the
 *  authored side is the visible one — no flip, which is what keeps the head on top there. */
function facingDepth(depth: number, facing: number): number {
  return facing === 2 ? -depth : depth;
}

/** A well-distributed 32-bit hash → `[0, 1)` — a stable per-instance material seed from the
 *  pawn's `entityReference`, so instances differ without the noise pattern swimming as it walks. */
function hash01(a: number): number {
  a = Math.imul(a ^ (a >>> 16), 0x45d9f3b);
  a = Math.imul(a ^ (a >>> 16), 0x45d9f3b);
  return ((a ^ (a >>> 16)) >>> 0) / 4294967296;
}


/** One drawn part SLOT of a live pawn — its warm prim + the last-baked fields the eps/skip
 *  compare gates on (human-pawns P3). `parts[0]` is the carrier (selection, hit-testing). */
interface PartPrim {
  id: number;
  x: number;
  y: number;
  w: number;
  texName: string | undefined;
  flipX: boolean;
  tint: number;
  geoColor: number;
  zIndex: number;
}

/** One live pawn: its warm PART prims + zone (so a zone close drops it), the last-synced spec
 *  (so an unchanged update skips the warm re-bake), the last AUTHORITATIVE tile (speculation's
 *  seed), the live speculation, if any, and the RENDER-CHASE track (movement-hardening F6). */
interface Mover {
  parts: PartPrim[];
  macroPosition: number;
  kind: number;
  /** The pawn's own packed def — its variant nibble dresses any slot with no payload entry. */
  def: number;
  authX: number;
  authY: number;
  spec: Walk | null;
  /** The RENDERED fractional tile — a separate track that CHASES the speculation target
   *  (user design, movement-hardening F6): the intent arrives ~4–5 tics late, so rendering
   *  the mathematically-correct spec directly SNAPS the pawn into the future. The chase
   *  closes the gap at ≤ {@link CHASE_CAP}× the pawn's true speed — subtle enough not to
   *  read as too-fast — and snaps only past {@link CHASE_SNAP_TILES}. */
  rx: number;
  ry: number;
  /** Last rx/ry actually BAKED into the warm prim — the eps gate compares against these, so
   *  sub-eps per-frame chase steps ACCUMULATE instead of being discarded (gating the chase
   *  state itself on eps froze the render until the snap threshold — the bug this split
   *  fixes). */
  arx: number;
  ary: number;
  /** Facing last applied to the prim. speculative-direction P1: in motion this derives
   *  from the RENDERED per-tick delta (never the server, never the spec-space walk — I1);
   *  at rest the server's facing is adopted from rows that arrive while resting (F1). */
  facing: number;
  /** When the facing last changed (ms) — the flip-hysteresis hold reads this. */
  lastFaceMs: number;
  /** The freshest intent tic ever armed — dedups replayed/duplicate `event` deliveries even
   *  after the spec cleared (zone re-subscribes replay history — first-pawns I2). */
  lastIntentTic: number | null;
  /** The freshest AUTHORITATIVE row tic seen for this pawn. Guards two things
   *  (spawn-authority P5, the teleport verdict): an intent serially older than the last
   *  auth row is HISTORY replayed by a zone re-subscribe — rejected even on a freshly
   *  recreated mover whose `lastIntentTic` is null (the hole the teleport hid in); and
   *  the AUTH probe scales its stride threshold by the tic gap between rows. */
  authTic: number | null;
  /** trait-lights P4/F8: the SPILL light prims — lights past the part count ride
   *  light-only sibling prims anchored by `carrierOf` (the reconciler re-derives their
   *  position from the carrier every frame, so they never need moving; they only need
   *  removing with the pawn). */
  lightPrims: number[];
  /** The lights' identity last built — a change (a payload trait joined, a hot-swap
   *  retuned the corpus) rebuilds the pawn's prims so every tuple re-attaches. */
  lightSig: string;
}

/** Render-chase cap: the render may move at most this multiple of the pawn's true speed
 *  (learned tic rate / tics-per-tile). +20% — the user's "hard for the human eye" bound. */
const CHASE_CAP = 1.2;
/** Non-linearity: catch-up factor is `1 + gap · this`, clamped to {@link CHASE_CAP} — a
 *  trailing render leans harder the further it lags, without ever breaking the cap. */
const CHASE_GAIN = 0.5;
/** Past this gap (tiles) the chase is hopeless — snap (tab was hidden, a teleport, …). */
const CHASE_SNAP_TILES = 3;
/** Per-frame dt clamp (ms) so a stalled rAF can't manufacture a huge chase step. */
const CHASE_DT_MAX_MS = 250;

/** speculative-direction P1: the facing dead-zone — a rendered per-tick step smaller than
 *  this (tiles) is noise and never turns the sprite (a wolf frame-step is ~0.008 tiles). */
const FACE_DEADZONE = 0.002;
/** Flip hysteresis: a facing holds at least this long before it may change again, so the
 *  greedy path's stair-steps and chase jitter cannot flicker the sprite. */
const FACE_HOLD_MS = 150;
/** Axis-dominance margin: an axis takes the facing only when its delta exceeds the other's
 *  by this factor. A PURE diagonal glide (Chebyshev steps advance both axes equally) is a
 *  knife-edge under float noise — without the margin the sprite oscillated every hold
 *  interval; with it, neither axis dominates and the current facing simply persists. */
const FACE_DOMINANCE = 1.3;

/** Serial u16 comparison: is `a` strictly newer than `b` on the wrapping tic ring? */
function ticNewer(a: number, b: number): boolean {
  return ((((a - b) & 0xffff) << 16) >> 16) > 0;
}

/** spawn-authority P5 (F5/I7): one recorded teleport-probe event. `kind` separates the
 *  two failures the word "teleport" conflates — an AUTH event is the authoritative row
 *  itself jumping beyond a hop stride (a server bug); a RENDER event is the chase
 *  snapping past {@link CHASE_SNAP_TILES} (a presentation bug). */
interface TeleportEvent {
  kind: "AUTH" | "RENDER";
  wallMs: number;
  entity: number;
  tic: number;
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  dist: number;
  specActive: boolean;
}

/** shared-simulation P0: one KIND's divergence samples — the numbers that say whether the
 *  client's walk and the server's still agree. Kept PER KIND because pace is per kind (a
 *  bunny at 24 tics/tile and a wolf at 12 pooled into one distribution hide each other),
 *  and as raw samples because the quantiles are the point — a mean over a bimodal split
 *  reads as "mildly off" when the truth is "two populations, one of them broken". */
interface KindDivergence {
  /** The client's DERIVED pace when the sample was taken (tics/tile) — its own belief. */
  pace: number;
  /** `|spec belief − authoritative point|` at each reseed, in tiles (Chebyshev, the metric
   *  the chase uses to decide whether to snap). */
  reseedErr: number[];
  /** Tiles between consecutive authoritative points — one re-anchor's observed stride.
   *  Euclidean, because the worker's own stride is a `hypot`. */
  anchorStride: number[];
  /** Tics between those same two rows — the re-anchor cadence as OBSERVED, not assumed. */
  anchorGap: number[];
  /** `gap / stride` per sample: the pace the server's own rows imply. Compared against
   *  {@link KindDivergence.pace}, this is the whole of shared-simulation I1 in one number. */
  impliedPace: number[];
  reseeds: number;
  /** Reseeds whose error exceeded {@link CHASE_SNAP_TILES} — the visible teleports. */
  snaps: number;
  landings: number;
}

/** The probe's ring buffer + counters, exposed as `__teleportProbe`. */
interface TeleportProbe {
  auth: number;
  render: number;
  events: TeleportEvent[];
  /** shared-simulation P0: per-kind divergence samples; {@link TeleportProbe.report}
   *  prints their quantiles, {@link TeleportProbe.reset} starts a fresh soak. */
  divergence: Record<number, KindDivergence>;
  report: () => Record<string, unknown>;
  reset: () => void;
}

/** Per-kind sample cap. Quantiles over the most recent N are representative and the arrays
 *  stay bounded across a long soak (90 movers re-anchoring every ~32 tics is ~17 rows/s). */
const DIVERGENCE_CAP = 4000;
/** shared-simulation P0: how often the tally mirrors into `sessionStorage` (ms). The webgl
 *  dev server full-reloads on HMR, which silently restarted the distribution mid-soak and
 *  cost two measurement runs — the tally now resumes instead. */
const DIVERGENCE_SAVE_MS = 2000;
/** The `sessionStorage` key the tally survives a reload in. */
const DIVERGENCE_KEY = "rd.divergence";

/** The fastest legal authoritative pace the probe allows (tiles/tic) — comfortably above
 *  any authored ground_speed (the fastest pawns run ~8 tics/tile = 0.125). */
const AUTH_JUMP_TILES_PER_TIC = 0.15;
/** Flat slack on top of the paced allowance (subtile rounding, the first row, resolves). */
const AUTH_JUMP_SLACK_TILES = 1.5;
/** An intent this many tics older than the pawn's freshest authoritative row is a zone
 *  re-subscribe replaying history, not a live order (the teleport verdict — spawn-authority
 *  P5). A LIVE intent's event tic trails the newest row by only the queue barrier (~4-5
 *  tics); replays trail by hundreds. */
const INTENT_STALE_BEHIND_AUTH_TICS = 16;
/** Ring cap so a pathological burst can't grow the probe without bound. */
const PROBE_RING_CAP = 200;

export class MoverLayer {
  private readonly movers = new Map<number, Mover>();
  /** Intents that arrived before their pawn's first `State` or before the tic clock anchored
   *  — HELD, never dropped (pawn-movement I1: a dropped live intent degrades the whole trip
   *  to its two authoritative snaps). {@link tick} re-evaluates them once both preconditions
   *  hold; the stale/dedup guards then arm or discard. Freshest per entity wins. */
  private readonly pendingIntents = new Map<number, MoveIntent>();
  private readonly unsubs: Array<() => void> = [];
  /** The teleport probe (spawn-authority P5) — see {@link TeleportProbe}. */
  private readonly teleportProbe: TeleportProbe = {
    auth: 0,
    render: 0,
    events: [],
    divergence: {},
    report: () => this.divergenceReport(),
    reset: () => this.divergenceReset(),
  };
  /** Wall-clock of the last `sessionStorage` mirror — throttles {@link saveDivergence}. */
  private lastDivergenceSaveMs = 0;
  /** Per-kind texture-stem / size / packed-channel tables from the content bundle, indexed by
   *  `objKind - 1` (same tables + indexing WorldBridge uses for cold things). Refreshed on
   *  hot-swap. */
  private thingStems: string[] = [];
  private thingLayout: Float64Array = new Float64Array();
  private thingPacked: Float64Array = new Float64Array();
  /** Per-kind authored tics-per-tile (`0` = unauthored → {@link defaultTicsPerTile}). */

  /** Payload part slots per entity (`slot → def`), joined from `PawnParts` events — either
   *  side of the join may arrive first (human-pawns P3). */
  private readonly pawnDefs = new Map<number, Map<number, number>>();
  // The payload and need rows are CORE's (shared-simulation P2d/P4). This layer used to keep
  // a parallel copy of the same fan — the fifth of five in the tree — and now reads them back
  // through the client, so the browser and a headless brain answer from one store.

  /** The world-view pathability probe (pathfinding I1), injected by the scene (the layer
   *  never reaches for the bridge). `null` = pathless speculation (the greedy line). */
  pathProbe: ((x: number, y: number) => boolean) | null = null;

  constructor(
    private readonly client: WasmClient,
    private content: Content,
    private readonly viewport: Viewport,
  ) {
    // Debug affordance (mirrors `__client`/`__content`): the layer + its mover map, so the
    // console can probe render-chase state (rx/ry vs spec target) and even drive `tick()`
    // in a hidden tab (no rAF) without a build.
    (globalThis as unknown as { __movers: Map<number, Mover> }).__movers = this.movers;
    (globalThis as unknown as { __moverLayer: MoverLayer }).__moverLayer = this;
    // spawn-authority P5 (F5/I7): the TELEPORT probe — AUTH events (the row itself
    // jumped) vs RENDER events (the chase snapped), timestamped, ring-buffered. Read
    // `__teleportProbe` in the console; the drill script polls it.
    (globalThis as unknown as { __teleportProbe: TeleportProbe }).__teleportProbe =
      this.teleportProbe;
    // shared-simulation P0: resume the divergence tally across a reload, and flush on the
    // way out so the last window isn't lost. `pagehide` (not `beforeunload`) — it fires on
    // the bfcache path too, which is how a dev-server HMR reload leaves.
    this.restoreDivergence();
    addEventListener("pagehide", () => this.saveDivergence(true));
    this.refreshTables();
    this.unsubs.push(client.onStateObject((obj) => this.onStateObject(obj)));
    this.unsubs.push(client.onPawnParts((p) => this.onPawnParts(p)));
    // `onPawnNeed` no longer folds here: core keys these rows by
    // `codec::object::row_reference` (P2d) and this layer reads them back. The old fold
    // re-spelled the 48-bit law as `need % 0x100000000` — one law, one spelling.
    this.unsubs.push(client.onMoveIntent((intent) => this.onMoveIntent(intent)));
    // A zone leaving the subscription sends no per-entity delete, so drop its movers.
    this.unsubs.push(client.onZoneClosed((macroPosition) => this.onZoneClosed(macroPosition)));
  }

  /** A content hot-swap: re-point the stem/layout/packed tables at the new bundle. */
  setContent(content: Content): void {
    this.content = content;
    this.refreshTables();
  }

  dispose(): void {
    for (const u of this.unsubs) u();
    this.unsubs.length = 0;
    for (const m of this.movers.values()) {
      for (const p of m.parts) this.viewport.warmRemovePrim(p.id);
    }
    this.movers.clear();
    this.pawnDefs.clear();
  }

  /** Wall-clock of the previous {@link tick} — the chase integrates real dt. */
  private lastTickMs = 0;

  /** The mover's emitted LIGHTS (trait-lights P4): the merged traits' tuples through
   *  the wasm `objectLights` (stride 9 — the ONE accessor, F5), shaped for the prim
   *  record. `hot` is FORCED — a mover's light anchor moves every frame, whatever the
   *  corpus says. `fall_off` (slot 5) is authored-not-yet-consumed (I10). */
  private moverLights(kind: number, entity: number): PrimitiveLight[] {
    const payload = this.client.pawnPayload(entity);
    const flat = this.content.objectLights(kind, payload);
    const out: PrimitiveLight[] = [];
    for (let i = 0; i + 8 < flat.length; i += 9) {
      const reach = flat[i + 4];
      if (!(reach > 0)) continue; // reach IS the "no light" test — the shared law
      const flags = flat[i + 8];
      out.push({
        color: [flat[i], flat[i + 1], flat[i + 2]],
        intensity: flat[i + 3],
        reach: reach * SQUARE,
        emitterRadius: flat[i + 7] * SQUARE,
        height: flat[i + 6] * SQUARE, // authored ELEVATION — the torch emits from its flame
        castShadows: (flags & 1) !== 0,
        hot: true,
        flicker: (flags & 4) !== 0,
      });
    }
    return out;
  }

  /** Record a teleport-probe event (ring-capped) and mirror it to the console so a
   *  soak's timeline reads straight out of devtools (spawn-authority P5). */
  private recordTeleport(e: TeleportEvent) {
    const p = this.teleportProbe;
    if (e.kind === "AUTH") p.auth += 1; else p.render += 1;
    p.events.push(e);
    if (p.events.length > PROBE_RING_CAP) p.events.shift();
    console.warn(
      `[teleport] ${e.kind} entity=${e.entity.toString(16)} ` +
      `(${e.fromX.toFixed(2)},${e.fromY.toFixed(2)})->(${e.toX.toFixed(2)},${e.toY.toFixed(2)}) ` +
      `dist=${e.dist.toFixed(2)} spec=${e.specActive} tic=${e.tic}`,
    );
  }

  // ── the divergence tally (shared-simulation P0) ────────────────────────────────────────
  //
  // The client walks a pawn from one authoritative row to the next; the worker walks the same
  // pawn from its own copy of the same rule. These samples are how we tell whether the two
  // still agree — and, once the rule lives in `shared/`, how we prove the fix.

  /** This kind's bucket, created on first sample. `pace` tracks the client's CURRENT belief
   *  (a pawn's derived speed moves with its conditions), not the first one it ever held. */
  private divergenceFor(kind: number, pace: number): KindDivergence {
    const d = this.teleportProbe.divergence;
    const k = d[kind] ?? (d[kind] = {
      pace, reseedErr: [], anchorStride: [], anchorGap: [], impliedPace: [],
      reseeds: 0, snaps: 0, landings: 0,
    });
    k.pace = pace;
    return k;
  }

  /** Push a sample, dropping the oldest past {@link DIVERGENCE_CAP}. */
  private static push(a: number[], v: number): void {
    if (!Number.isFinite(v)) return;
    a.push(v);
    if (a.length > DIVERGENCE_CAP) a.splice(0, a.length - DIVERGENCE_CAP);
  }

  /** Quantile of an UNSORTED sample array — sorts a copy, because the report is read by
   *  hand and the samples are written thousands of times more often than they're read. */
  private static quantile(a: number[], f: number): number | null {
    if (!a.length) return null;
    const s = [...a].sort((x, y) => x - y);
    return +s[Math.min(s.length - 1, Math.floor(f * s.length))].toFixed(3);
  }

  /** One authoritative row landing on a pawn that already had one: the observed stride and
   *  cadence, plus the pace those two imply. Skips the resting case — a pawn that didn't
   *  move carries no information about pace and would drag every quantile to zero. */
  private noteAnchor(kind: number, pace: number, stride: number, gap: number): void {
    if (gap <= 0 || stride < 0.05) return;
    const d = this.divergenceFor(kind, pace);
    MoverLayer.push(d.anchorStride, stride);
    MoverLayer.push(d.anchorGap, gap);
    MoverLayer.push(d.impliedPace, gap / stride);
    this.saveDivergence();
  }

  /** One reseed or landing: how far the client's belief had drifted when truth arrived. */
  private noteReseed(kind: number, pace: number, err: number, landed: boolean): void {
    const d = this.divergenceFor(kind, pace);
    MoverLayer.push(d.reseedErr, err);
    if (landed) d.landings += 1; else d.reseeds += 1;
    if (err > CHASE_SNAP_TILES) d.snaps += 1;
    this.saveDivergence();
  }

  /** `__teleportProbe.report()` — the divergence quantiles per kind, in one read.
   *  `clientPace` vs `impliedPace` is the comparison the whole stream turns on: equal means
   *  the two walks agree, and a ratio means they don't. */
  divergenceReport(): Record<string, unknown> {
    const Q = MoverLayer.quantile;
    const live: Record<number, number> = {};
    for (const m of this.movers.values()) live[m.kind] = (live[m.kind] ?? 0) + 1;
    const out: Record<string, unknown> = {};
    for (const [key, d] of Object.entries(this.teleportProbe.divergence)) {
      const kind = Number(key);
      out[`${kind} · ${this.thingStems[kind - 1] ?? "unknown"}`] = {
        movers: live[kind] ?? 0,
        clientPace: d.pace,
        impliedPace: { p10: Q(d.impliedPace, 0.1), p50: Q(d.impliedPace, 0.5), p90: Q(d.impliedPace, 0.9) },
        anchorStride: { p50: Q(d.anchorStride, 0.5), p90: Q(d.anchorStride, 0.9) },
        anchorGapTics: { p50: Q(d.anchorGap, 0.5), p90: Q(d.anchorGap, 0.9) },
        reseedErrTiles: { p50: Q(d.reseedErr, 0.5), p90: Q(d.reseedErr, 0.9), max: Q(d.reseedErr, 1) },
        reseeds: d.reseeds,
        landings: d.landings,
        pastSnapThreshold: d.snaps,
        pastSnapPct: d.reseeds + d.landings
          ? +((100 * d.snaps) / (d.reseeds + d.landings)).toFixed(1) : 0,
        samples: d.anchorStride.length,
      };
    }
    // Named away from `auth`/`render` here: these read as a verdict, not as the probe's
    // internal field names, and one of them trips console-tooling key filters.
    out._teleports = { rowJumps: this.teleportProbe.auth, chaseSnaps: this.teleportProbe.render };
    return out;
  }

  /** `__teleportProbe.reset()` — start a fresh soak, storage included. */
  divergenceReset(): void {
    this.teleportProbe.divergence = {};
    this.teleportProbe.auth = 0;
    this.teleportProbe.render = 0;
    this.teleportProbe.events.length = 0;
    this.saveDivergence(true);
    console.info("[divergence] tally reset");
  }

  /** Mirror the tally into `sessionStorage`, throttled to {@link DIVERGENCE_SAVE_MS}. */
  private saveDivergence(force = false): void {
    const now = Date.now();
    if (!force && now - this.lastDivergenceSaveMs < DIVERGENCE_SAVE_MS) return;
    this.lastDivergenceSaveMs = now;
    try {
      sessionStorage.setItem(DIVERGENCE_KEY, JSON.stringify({
        divergence: this.teleportProbe.divergence,
        auth: this.teleportProbe.auth,
        render: this.teleportProbe.render,
      }));
    } catch {
      // Quota, or a session with storage denied. The tally is a probe: never load-bearing.
    }
  }

  /** Resume a tally left by the previous page. A shape change from an older build starts
   *  clean rather than throwing — a stale probe must never break the client. */
  private restoreDivergence(): void {
    try {
      const raw = sessionStorage.getItem(DIVERGENCE_KEY);
      if (!raw) return;
      const s = JSON.parse(raw) as
        { divergence?: Record<number, KindDivergence>; auth?: number; render?: number };
      if (!s.divergence) return;
      this.teleportProbe.divergence = s.divergence;
      this.teleportProbe.auth = s.auth ?? 0;
      this.teleportProbe.render = s.render ?? 0;
      const n = Object.values(s.divergence)
        .reduce((a, d) => a + (d.anchorStride?.length ?? 0), 0);
      if (n) console.info(`[divergence] resumed ${n} anchor sample(s) across a reload`);
    } catch {
      this.teleportProbe.divergence = {};
    }
  }

  /** Per-frame: re-evaluate held intents whose preconditions now hold, then advance every
   *  live speculation along its greedy line at the tic estimate, and CHASE it with the
   *  rendered position (F6 — the spec is truth-tracking, the chase is presentation). */
  tick(): void {
    if (this.pendingIntents.size) {
      for (const [key, intent] of this.pendingIntents) {
        if (this.movers.has(key) && this.client.ticDelta(intent.eventTic) !== null) {
          this.pendingIntents.delete(key);
          this.onMoveIntent(intent); // ONE evaluation — the guards arm or discard it
        }
      }
    }
    const now = Date.now();
    const dtSec = Math.min(CHASE_DT_MAX_MS, this.lastTickMs ? now - this.lastTickMs : 16) / 1000;
    this.lastTickMs = now;
    for (const [key, m] of this.movers) {
      // The TARGET: the speculated position while a spec lives, the resting authoritative
      // tile otherwise (post-landing the chase closes the last fraction of a tile).
      let tx = m.authX;
      let ty = m.authY;
      let facing = m.facing;
      // WHERE THE PAWN IS comes from CORE (shared-simulation P4). This was a TypeScript
      // re-implementation of the worker's walk under a comment claiming it "mirrored EXACTLY"
      // — a claim nothing checked, and false in at least five ways (`walk-divergence.md`).
      // What remains below is the chase, because how fast a sprite ADMITS a correction is the
      // only part of this a viewer legitimately owns (F2).
      const core = this.client.pawnPoint(key);
      if (core) {
        tx = core[0];
        ty = core[1];
        if (m.spec) {
          m.spec.appliedX = core[0]; // the probe still measures belief-vs-truth
          m.spec.appliedY = core[1];
        }
      }
      // Chase: close the render→target gap at ≤ CHASE_CAP × true speed, leaning harder the
      // further behind (CHASE_GAIN), snapping past CHASE_SNAP_TILES. Chebyshev metric and
      // per-axis stepping match the greedy path's diagonal geometry.
      const prevRx = m.rx;
      const prevRy = m.ry;
      const gx = tx - m.rx;
      const gy = ty - m.ry;
      const gap = Math.max(Math.abs(gx), Math.abs(gy));
      if (gap > 0) {
        // The pace is CORE's derivation; the chase only needs the rate to cap itself (F2).
        const tilesPerSec = this.client.ticsPerSec() / (this.client.pawnPace(key) ?? defaultTicsPerTile());
        if (gap > CHASE_SNAP_TILES) {
          // The RENDER probe (spawn-authority P5): the visible teleport — the chase gave
          // up and snapped. Recorded with the belief→target pair so it correlates (or
          // fails to) with an AUTH event at the same wall time.
          this.recordTeleport({
            kind: "RENDER", wallMs: now, entity: key, tic: 0,
            fromX: m.rx, fromY: m.ry, toX: tx, toY: ty,
            dist: gap, specActive: m.spec !== null,
          });
        }
        const step = gap > CHASE_SNAP_TILES
          ? gap // hopeless — snap
          : tilesPerSec * Math.min(CHASE_CAP, 1 + gap * CHASE_GAIN) * dtSec;
        // The chase STATE always advances (sub-eps steps accumulate); only the warm
        // re-bake is eps-gated, against the last APPLIED position.
        m.rx += Math.sign(gx) * Math.min(step, Math.abs(gx));
        m.ry += Math.sign(gy) * Math.min(step, Math.abs(gy));
      }
      // SPECULATIVE DIRECTION (P1, the user's model): the sprite faces the way it is
      // ACTUALLY RENDERED moving — the per-tick delta of the chase position. Dead-zone
      // kills noise, the dominant axis wins, a tie keeps the current facing, and a flip
      // holds FACE_HOLD_MS before the next (the stair-step cannot flicker). At rest the
      // delta is zero and the facing simply persists (server turns arrive via F1).
      const mdx = m.rx - prevRx;
      const mdy = m.ry - prevRy;
      const adx = Math.abs(mdx);
      const ady = Math.abs(mdy);
      if (Math.max(adx, ady) >= FACE_DEADZONE && now - m.lastFaceMs >= FACE_HOLD_MS) {
        const cand = adx >= ady * FACE_DOMINANCE ? (mdx > 0 ? 1 : 3)
                   : ady >= adx * FACE_DOMINANCE ? (mdy > 0 ? 0 : 2)
                   : facing;                       // near-diagonal: neither dominates — keep
        if (cand !== facing) {
          facing = cand;
          m.lastFaceMs = now;
        }
      }
      // BOOT-RACE HEAL (lighting-correctness P3): a mover whose first applyVisual ran before
      // the content bundle streamed got a single fallback part — and a RESTING mover never
      // re-applies, so it stayed textureless forever. When the kind's slot count disagrees
      // with the built parts, re-apply regardless of the eps gate.
      if (m.parts.length !== this.slotCountFor(m.kind)) {
        this.applyVisual(m, key, m.kind, m.def, m.rx, m.ry, facing, m.macroPosition);
        continue;
      }
      if (
        Math.abs(m.rx - m.arx) >= RENDER_APPLY_EPS || Math.abs(m.ry - m.ary) >= RENDER_APPLY_EPS ||
        facing !== m.facing
      ) {
        if (facing !== m.facing) {
          // speculative-direction P0: name every turn — old→new, source, and where the RENDER
          // actually moved this frame (the delta the eye follows).
          console.debug(
            `[facing] ${m.facing}->${facing} src=motion entity=${key.toString(16)} ` +
            `d=(${mdx.toFixed(3)},${mdy.toFixed(3)}) r=(${m.rx.toFixed(2)},${m.ry.toFixed(2)})`,
          );
        }
        m.arx = m.rx;
        m.ary = m.ry;
        m.facing = facing;
        this.applyVisual(m, key, m.kind, m.def, m.rx, m.ry, facing, m.macroPosition);
      }
    }
  }

  /** ui-select P1: the warm prim id for a pawn entity — the CARRIER (slot 0) prim; details
   *  follow it. */
  primIdOf(entity: number): number | null {
    return this.movers.get(entity)?.parts[0]?.id ?? null;
  }

  /** human-pawns-redux P1 (I4): EVERY part's prim id, slot order — the outline set walks
   *  all of them so a selected human's head is marked with its body. */
  partPrimIdsOf(entity: number): number[] {
    return this.movers.get(entity)?.parts.map((p) => p.id) ?? [];
  }

  /** ui-select P2: a pawn's live details for the details panel — authoritative tile, facing,
   *  content speed, zone address, and whether a speculation is in flight. */
  pawnInfo(entity: number): {
    kind: number; stem: string; tileX: number; tileY: number; facing: number;
    macroPosition: number; moving: boolean; ticsPerTile: number;
  } | null {
    const m = this.movers.get(entity);
    if (!m) return null;
    return {
      kind: m.kind, stem: this.thingStems[m.kind - 1] ?? "?",
      // The DISPLAY tile is the floor of the fractional point (chord-movement I2 —
      // client flooring is advisory; the worker validates on its own resolves).
      tileX: Math.floor(m.authX), tileY: Math.floor(m.authY), facing: m.facing,
      macroPosition: m.macroPosition, moving: !!m.spec, ticsPerTile: this.client.pawnPace(entity) ?? 0,
    };
  }

  /** ui-select P0 (D2): the topmost MOVER whose drawn box contains the world point (px) —
   *  painter order (zIndex, then southernmost) among overlaps. Returns the stable ENTITY
   *  reference (warm prim ids churn on despawn); null → no pawn there. */
  pawnAt(wx: number, wy: number): { entity: number; primId: number } | null {
    let best: { entity: number; primId: number; zIndex: number; y: number } | null = null;
    for (const [key, m] of this.movers) {
      // ANY part's drawn box hits (a click on the head selects the pawn); the returned
      // prim id is always the CARRIER's (the selection outline follows slot 0).
      const carrier = m.parts[0];
      if (!carrier) continue;
      for (const part of m.parts) {
        const p = this.viewport.warmGetPrim(part.id);
        if (!p) continue;
        if (wx < p.x || wx >= p.x + p.width || wy < p.y || wy >= p.y + p.height) continue;
        if (!best || p.zIndex > best.zIndex || (p.zIndex === best.zIndex && p.y > best.y)) {
          best = { entity: key, primId: carrier.id, zIndex: p.zIndex, y: p.y };
        }
      }
    }
    return best ? { entity: best.entity, primId: best.primId } : null;
  }

  /** The payload's part slots landed/changed for an entity — join them to a live mover
   *  (either side may arrive first; a pre-mover payload just waits in the map). The RAW
   *  stream is kept too (needs-moodlets P5): the details panel feeds it verbatim to the
   *  wasm `pawnConditions` eval — this layer never decodes NEED/CONDITION entries. */
  private onPawnParts(p: { entityReference: number; parts: { slot: number; def: number }[]; payload: Uint32Array }): void {
    const map = new Map<number, number>();
    for (const e of p.parts) map.set(e.slot, e.def);
    this.pawnDefs.set(p.entityReference, map);
    const m = this.movers.get(p.entityReference);
    if (m) this.applyVisual(m, p.entityReference, m.kind, m.def, m.rx, m.ry, m.facing, m.macroPosition);
  }

  /** The latest RAW payload opcode stream fanned for `entity`, or null — the details
   *  panel's eval input (needs-moodlets P5). */
  pawnPayload(entity: number): Uint32Array | null {
    const p = this.client.pawnPayload(entity);
    return p.length ? p : null;
  }

  /** The entity's `needs` rows, flattened stride-2 `[packed, setTic, …]` (stat-model F2) —
   *  the second eval input; fed verbatim to `pawnConditions`/`pawnEmotion`/`needState`.
   *
   *  **Float64Array, not Uint32Array.** A need row is the ONE u64 shape
   *  `dead:16 | data:16 | reference:32` (trait-rows-u32 F1) — 48 significant bits, which a JS
   *  number holds exactly under THE 48-BIT LAW but a `Uint32Array` silently truncates. It used
   *  to be `Uint32Array`, which cut `0xa4fb80010020` down to `0x80010020` and threw away the
   *  `data` half — i.e. the need's VALUE. Every consumer therefore read every need as 0, so
   *  band-derived conditions (thirsty / hungry / starving) could never fire on the client and
   *  the live-edit needs bars were all empty. Found 2026-08-09 by probing the raw rows when
   *  those bars read zero on a healthy pawn (live-edit I9). */
  pawnNeeds(entity: number): Float64Array {
    // Stride-2 `Float64Array` still, for the reason recorded above — a need row is a u64 and the
    // u32 container truncated it, so every need read zero. The ROWS now come from core.
    return this.client.pawnNeeds(entity);
  }

  // ── internals ───────────────────────────────────────────────────────

  private refreshTables(): void {
    this.thingStems = this.content.thingTextureStems();
    this.thingLayout = this.content.thingLayout();
    this.thingPacked = this.content.thingPackedChannels();
    this.geoLabels = this.content.thingGeoLabels();
    this.slotCounts.clear();
  }

  /** survival F1: per-kind GEO glyphs (object_id order), refreshed on hot-swap. */
  private geoLabels: string[] = [];

  /** Slot count per kind, cached (cleared on content swap) — the RESTING-mover heal below
   *  compares against it every tick, and a wasm call per mover per frame would be churn. */
  private readonly slotCounts = new Map<number, number>();

  private slotCountFor(kind: number): number {
    let n = this.slotCounts.get(kind);
    if (n === undefined) {
      n = (this.content.moverParts(kind) as unknown[]).length;
      this.slotCounts.set(kind, n);
    }
    return n;
  }

  /** Slice a pawn's up-to-4 packed-channel material bindings out of the stride-8 per-def table
   *  by `objKind` (mirrors {@link WorldBridge}'s `packedFor`), or `undefined` when the def binds
   *  no material — the common case for pawns today (the wolf authors none → flat reconstruction). */
  private packedFor(kind: number): PackedChannel[] | undefined {
    const table = this.thingPacked;
    const base = (kind - 1) * 8;
    if (base < 0 || base + 8 > table.length) return undefined;
    let bound = false;
    const channels: PackedChannel[] = [];
    for (let i = 0; i < 4; i++) {
      const materialId = table[base + i * 2];
      const tint = table[base + i * 2 + 1];
      channels.push({ materialId, tint });
      if (materialId > 0) bound = true;
    }
    return bound ? channels : undefined;
  }

  /** A promoted move intent: start (or retarget) the entity's speculation from its last
   *  authoritative tile. An intent for a pawn we haven't seen yet, or one arriving before the
   *  tic clock anchors, is HELD in {@link pendingIntents} (never dropped — I1) and
   *  re-evaluated by {@link tick} once its preconditions hold. */

  private onMoveIntent(intent: MoveIntent): void {
    const m = this.movers.get(intent.entityReference);
    const d = this.client.ticDelta(intent.eventTic);
    if (!m || d === null) {
      const held = this.pendingIntents.get(intent.entityReference);
      if (!held || ticNewer(intent.eventTic, held.eventTic)) {
        this.pendingIntents.set(intent.entityReference, intent);
      }
      return;
    }
    const tpt = this.client.pawnPace(intent.entityReference) ?? defaultTicsPerTile();
    // The `event` table replays HISTORY on subscribe (no retention yet — first-pawns I2), and
    // deliveries can arrive out of order — so guard: an intent whose move must already be over
    // ⇒ the authoritative rows carry the outcome; an intent serially older than the live spec
    // ⇒ superseded. Only a fresh, live intent arms speculation.
    const span = Math.max(Math.abs(intent.tileX - m.authX), Math.abs(intent.tileY - m.authY));
    if (d > (span + 2) * tpt) return; // finished long ago — stale replay
    if (m.lastIntentTic !== null && ((((intent.eventTic - m.lastIntentTic) & 0xffff) << 16) >> 16) <= 0) {
      return; // a duplicate delivery or a serially older intent — superseded
    }
    // The teleport verdict (spawn-authority P5, found live): a zone crossing re-subscribes
    // the event stream, HISTORY replays (first-pawns I2), and the mover — freshly recreated
    // by the crossing, `lastIntentTic` null — re-armed a minutes-old intent. The spec then
    // walked the OLD dest while the server walked the new one; the divergence grew until
    // the chase snapped: the user-visible teleport. Authoritative rows outrank history: an
    // intent serially behind the freshest row by more than the queue barrier is a replay.
    if (m.authTic !== null
      && ((((m.authTic - intent.eventTic) & 0xffff) << 16) >> 16) > INTENT_STALE_BEHIND_AUTH_TICS) {
      console.debug(
        `[mover] intent REJECTED as replayed history entity=${intent.entityReference.toString(16)} ` +
        `eventTic=${intent.eventTic} authTic=${m.authTic}`,
      );
      return;
    }
    m.lastIntentTic = intent.eventTic;
    console.debug(
      `[mover] intent armed entity=${intent.entityReference.toString(16)} dest=(${intent.tileX},${intent.tileY}) tic=${intent.eventTic} d=${d.toFixed(1)}`,
    );
    // chord-movement F4: the seed fans NO position — speculation arms from the CURRENT
    // BELIEF (the rendered point), so an interrupted trip re-aims from where the pawn is
    // DRAWN and can never snap back to a tile. Re-anchors and the landing correct.
    const bx = m.rx;
    const by = m.ry;
    // The client no longer paths: core walks the pawn and webgl chases the answer (P4).
    let path: { x: number; y: number }[] | null = null;
    m.spec = {
      fromX: bx,
      fromY: by,
      destX: intent.tileX,
      destY: intent.tileY,
      eventTic: intent.eventTic,
      ticsPerTile: tpt,
      appliedX: bx,
      appliedY: by,
      path,
    };
    // speculative-direction P1: the INITIAL AIM — the pawn turns toward its path at the
    // seed (the greedy walk's e/w-first first leg), before any rendered delta exists. The
    // only facing the walk still contributes; every later turn is motion-derived.
    const aim = intent.tileX > m.authX ? 1 : intent.tileX < m.authX ? 3
              : intent.tileY > m.authY ? 0 : intent.tileY < m.authY ? 2 : m.facing;
    if (aim !== m.facing) {
      console.debug(`[facing] ${m.facing}->${aim} src=aim entity=${intent.entityReference.toString(16)}`);
      m.facing = aim;
      m.lastFaceMs = Date.now();
      this.applyVisual(m, intent.entityReference, m.kind, m.def, m.rx, m.ry, aim, m.macroPosition);
    }
  }

  private onStateObject(obj: StateObject): void {
    const key = obj.entityReference;
    if (obj.removed) {
      this.remove(key);
      return;
    }
    // Content kind → sprite. A CREATE-minted pawn carries a PACKED def (human-pawns P0:
    // `TYPE_PAWN | species | kind | variant` — a nonzero high half tells it apart), whose
    // kind_id is the content object_id; a legacy raw-object_id def is the id itself. A
    // legacy-placed pawn carries `definitionReference` 0 (`PLACE` has no def operand) — fall
    // back to the first thing kind.
    const def = obj.definitionReference;
    const kindId = def >>> 16 !== 0 ? (def & 0xffff) >>> 4 : def;
    const kind = kindId >= 1 && kindId <= this.thingStems.length ? kindId : 1;

    const m = this.movers.get(key);

    // The teleport verdict (spawn-authority P5): zone re-subscribes replay STATE history
    // too — a strictly-older row re-applied after a newer one drags authX/authY (and the
    // spec reseed) BACKWARDS, the twin of the replayed-intent bug. Rows apply in serial
    // order only; equal tics (multiple mutations per tic) still apply.
    if (m && m.authTic !== null && ticNewer(m.authTic, obj.tic)) {
      return;
    }

    // F8 — the authoritative row corrects speculation: log the observed error (the data the
    // re-anchor cadence is tuned on), then snap. The final tile clears the spec; an interim
    // authoritative resolve (another event touched the pawn) reseeds it.
    // The FRACTIONAL authoritative point (chord-movement F1): tile + sixteenths. Every
    // row is subtile-accurate now — mid-chord re-anchors land between tiles.
    const ax = obj.tileX + (obj.subX ?? 0) / 16;
    const ay = obj.tileY + (obj.subY ?? 0) / 16;
    let landedThisRow = false;
    if (m?.spec) {
      const s = m.spec;
      const err = Math.max(Math.abs(s.appliedX - ax), Math.abs(s.appliedY - ay));
      // shared-simulation P0: how far the client's belief had drifted when truth arrived.
      this.noteReseed(m.kind, s.ticsPerTile, err, obj.tileX === s.destX && obj.tileY === s.destY);
      if (obj.tileX === s.destX && obj.tileY === s.destY) {
        console.debug(
          `[mover] spec landed e=${err.toFixed(2)} tiles entity=${key.toString(16)} tic=${obj.tic}`,
        );
        m.spec = null;
        landedThisRow = true;
      } else {
        console.debug(
          `[mover] spec reseed e=${err.toFixed(2)} tiles entity=${key.toString(16)} tic=${obj.tic}`,
        );
        s.fromX = ax;
        s.fromY = ay;
        s.eventTic = obj.tic;
        s.appliedX = ax;
        s.appliedY = ay;
        // The worker's per-hop recompute, mirrored at the reseed cadence (I1): the rest
        // of the route re-chords from the fresh authoritative POINT (its floor tile).
        s.path = null; // core owns the route (P4)
      }
    }

    if (m) {
      // Existing mover: the authoritative row STEERS (auth point, zone, kind); the render
      // keeps chasing from wherever it is (F6 — no direct snap; `tick` closes the gap at
      // the capped rate, or snaps itself past the hopeless threshold).
      // The AUTH probe (spawn-authority P5): consecutive authoritative points for one
      // pawn further apart than the pawn could legally WALK in the tic gap = the ROW
      // jumped, whatever the render does. Scaled by the tic delta — rows only arrive
      // every ~32 tics mid-chord (the re-anchor cadence), so a flat threshold flags
      // ordinary walking (the probe's own first false positive, found live).
      const authJump = Math.max(Math.abs(ax - m.authX), Math.abs(ay - m.authY));
      const authDt = m.authTic === null ? 0 : (obj.tic - m.authTic) & 0xffff;
      const allowed = authDt > 0 && authDt < 0x8000
        ? AUTH_JUMP_TILES_PER_TIC * authDt + AUTH_JUMP_SLACK_TILES
        : AUTH_JUMP_SLACK_TILES;
      if (authJump > allowed) {
        this.recordTeleport({
          kind: "AUTH", wallMs: Date.now(), entity: key, tic: obj.tic,
          fromX: m.authX, fromY: m.authY, toX: ax, toY: ay,
          dist: authJump, specActive: m.spec !== null,
        });
      }
      // shared-simulation P0: the observed stride + cadence between two authoritative
      // points. EUCLIDEAN, because the worker's own hop stride is a `hypot` — measuring it
      // Chebyshev like the jump probe above would under-report every diagonal by up to 30%.
      if (m.authTic !== null) {
        this.noteAnchor(
          m.kind, this.client.pawnPace(key) ?? 0, Math.hypot(ax - m.authX, ay - m.authY),
          authDt < 0x8000 ? authDt : 0,
        );
      }
      m.authX = ax;
      m.authY = ay;
      if (m.authTic === null || ticNewer(obj.tic, m.authTic)) m.authTic = obj.tic;
      m.kind = kind;
      m.def = obj.definitionReference;
      m.macroPosition = obj.macroPosition;
      // speculative-direction F1: the server's facing turns a STANDING pawn — but only from
      // rows that ARRIVE while resting. The landing row's facing is stale by construction
      // (it raced the trip), and a mid-motion row steers position but never the sprite.
      const resting = !m.spec &&
        Math.max(Math.abs(ax - m.rx), Math.abs(ay - m.ry)) < 0.25;
      if (resting && !landedThisRow && obj.facing !== m.facing) {
        console.debug(`[facing] ${m.facing}->${obj.facing} src=server-rest entity=${key.toString(16)}`);
        m.facing = obj.facing;
        m.lastFaceMs = Date.now();
        this.applyVisual(m, key, kind, obj.definitionReference, m.rx, m.ry, obj.facing, obj.macroPosition);
      }
      return;
    }
    this.applyVisual(null, key, kind, obj.definitionReference, ax, ay, obj.facing, obj.macroPosition);
  }

  /** Create or mutate the pawn's warm PART prims at (possibly fractional) global tile
   *  `(tileX, tileY)` with `facing` — the shared path for authoritative rows, per-frame
   *  speculation, AND payload-join arrivals (human-pawns P3). Slot 0 boxes the carrier via
   *  the kind's layout; other slots place at `offset` tiles from its anchor at `scale` ×
   *  its size, drawing the payload `PART` def for their slot (else the pawn's own def). */
  private applyVisual(
    m: Mover | null,
    key: number,
    kind: number,
    def: number,
    tileX: number,
    tileY: number,
    facing: number,
    macroPosition: number,
  ): void {
    const slots = this.content.moverParts(kind) as MoverPart[];
    const slotDefs = this.pawnDefs.get(key);
    const has = (name: string) => this.viewport.hasTexture(name);
    const defOf = (slot: number): number => slotDefs?.get(slot) ?? def;
    // A payload def may redirect a slot to ANOTHER kind's stem (the armor future); today
    // the human's PART defs name its own kind, so this resolves to the slot's DSL stem.
    const stemOf = (s: MoverPart, d: number): string | undefined => {
      const kindId = d >>> 16 !== 0 ? (d & 0xffff) >>> 4 : d;
      const fromDef = kindId >= 1 && kindId <= this.thingStems.length ? this.thingStems[kindId - 1] : undefined;
      return fromDef ?? s.stem ?? undefined;
    };
    const variantOf = (d: number): number => (d >>> 16 !== 0 ? d & 0xf : 0);

    // F4: a slot's authored `scale` is its stem's PRE-ATLAS sprite scale, registered under the
    // EXACT resolved name (I7: the kind-level registration keys a stem that never gets packed).
    // Registering before the prim is placed means the first pack already carries the transform.
    const scaleSlot = (s: MoverPart, name: string | undefined): void => {
      if (name) this.viewport.setSpriteScale(name, s.scale, s.scale, s.spriteAnchorX, s.spriteAnchorY);
    };
    // human-pawns-redux P1, closing subframe-ingest I11: the slot's authored subframe rect,
    // registered under the SAME resolved name. The rect is the [variant][rotation] cell the
    // loader composed (fallback chain included); a WEST facing draws the EAST master
    // mirrored, so it registers (and needs) east's rect — the resolved name IS the east
    // stem. Full-frame defaults no-op inside `setSubframe`, so unauthored slots stay on
    // the old path.
    const subframeSlot = (s: MoverPart, name: string | undefined, variant: number): void => {
      const sub = s.subframes;
      if (!name || !sub) return;
      const rot = facing === 3 ? 1 : facing; // w = mirrored e (s0 e1 n2 w3)
      const o = (variant * 16 + rot) * 6;
      if (o + 6 > sub.length) return;
      this.viewport.setSubframe(name, 0, sub[o], sub[o + 1], sub[o + 2], sub[o + 3], sub[o + 4], sub[o + 5]);
    };

    // Slot 0 — the carrier box from the kind's layout (the wolf's whole visual).
    const d0 = defOf(0);
    const layout = readLayout(this.thingLayout, kind);
    const tex0 = moverSlotTexture(stemOf(slots[0], d0), facing, variantOf(d0), slots[0].part, has);
    scaleSlot(slots[0], tex0.name);
    subframeSlot(slots[0], tex0.name, variantOf(d0));
    const box = placeThing(tileX, tileY, layout, tex0.flipX, !tex0.name);
    const size0 = box.width; // square box
    // The pawn's ROW band. Slots sort WITHIN it by their own facing-resolved depth (F1) — the
    // row still decides which pawn is in front, and the blit's warm-over-cold compare reads the
    // row out of `zdepth_world` (from `prim.y + height`), which no slot z touches.
    const orderRowBase = PAWN_ORDER_BASE + box.orderRow;
    // ── THE PART-ORDERING RULE (z-positioning F4) ───────────────────────────────────────────
    // Parts of one pawn order by the AUTHORED, facing-flipped `depth` — then by slot index as a
    // stable tiebreak. **Not by elevation**, and this is the load-bearing part of the rule:
    // `facingDepth` negates depth when the pawn faces north, so a head at `+1` draws OVER the body
    // from the front and UNDER it from behind. Elevation cannot express that — a head is the same
    // height whichever way the pawn is turned.
    //
    // Since z-positioning P2b, head and body share a base row (they stand in the same place), so
    // `orderRowBase` is equal for both and this expression IS what separates them. It was previously
    // true by accident of iteration order; it is now the rule, written where someone would change it.
    const slotOrder = (s: MoverPart, i: number): number =>
      orderRowBase + facingDepth(s.depth, facing) * SLOT_DEPTH_ORDER + i * SLOT_INDEX_ORDER;
    // The carrier's game anchor (base-centre) + the px-per-tile scale for slot offsets. Every box
    // is `span × SQUARE` now (F4 — the scale lives in the art), so one tile is `size0 / span`.
    const ax = box.x + size0 * 0.5;
    const ay = box.y + size0;
    const tilePx = layout.span > 0 ? size0 / layout.span : size0;

    interface SlotSpec {
      texName: string | undefined; flipX: boolean; x: number; y: number; w: number;
      tint: number; geoColor: number; zIndex: number;
      /** z-positioning P3: world px above the ground plane; `y` is already shifted north by it. */
      elevation?: number;
    }
    const specs: SlotSpec[] = [
      { texName: tex0.name, flipX: tex0.flipX, x: box.x, y: box.y, w: size0,
        tint: slots[0].tint, geoColor: slots[0].geoColor, zIndex: slotOrder(slots[0], 0) },
    ];
    for (let i = 1; i < slots.length; i++) {
      const s = slots[i];
      const di = defOf(i);
      const tex = moverSlotTexture(stemOf(s, di), facing, variantOf(di), s.part, has);
      scaleSlot(s, tex.name);
      subframeSlot(s, tex.name, variantOf(di));
      // The slot's OWN frame span in world px — never slot 0's box times a factor (F4): the drawn
      // box must equal the def's pow2 frame span or the lighting card mis-sizes by the ratio.
      const w = (s.span > 0 ? s.span : 1) * tilePx;
      // The slot's authored offset (tiles) from the carrier anchor; x mirrors with a west
      // facing so the part stays on the sprite's correct side.
      const ox = (tex.flipX ? -s.offsetX : s.offsetX) * tilePx;
      const cx = ax + ox;
      // z-positioning P3: elevation draws as a northward shift of the SAME size (F6 — "there is
      // no draw-side constant; the screen-north shift IS the elevation"). It is added here beside
      // offsetY because on screen they are indistinguishable; they part company at the record,
      // where `elevation` is carried separately so the ground position stays recoverable.
      const elevTiles = s.offsetZ ?? 0;
      const cy = ay + s.offsetY * tilePx - elevTiles * tilePx;
      specs.push({
        texName: tex.name, flipX: tex.flipX, x: cx - w * 0.5, y: cy - w * 0.5, w,
        tint: s.tint, geoColor: s.geoColor,
        zIndex: slotOrder(s, i),
        elevation: elevTiles * tilePx,   // world px above the ground plane
      });
    }

    // trait-lights P4: the mover's LIGHTS from its merged traits — part i carries
    // light i on its own prim (the torch pattern: billboard + light, one prim); lights
    // past the part count SPILL into light-only sibling prims (F8 — nothing drops).
    const lights = this.moverLights(kind, key);
    const lightSig = JSON.stringify(lights);

    // A slot-count change (a late payload join won't change the count — the DSL fixes it —
    // but a content hot-swap can), or the LIGHT SET changing (a payload trait joined, a
    // corpus retune): rebuild from scratch so every tuple re-attaches.
    if (m && (m.parts.length !== specs.length || m.lightSig !== lightSig)) {
      for (const p of m.parts) this.viewport.warmRemovePrim(p.id);
      for (const id of m.lightPrims) this.viewport.warmRemovePrim(id);
      m.parts = [];
      m.lightPrims = [];
    }

    if (!m || m.parts.length === 0) {
      // Sequential: slot 0 mints the pawn's CARRIER prim; the other slots attach to it as
      // carried pieces of the SAME `prim_data` record (P5 — "a pawn = prim{head, body, …}").
      const parts: PartPrim[] = [];
      for (let i = 0; i < specs.length; i++) {
        const sp = specs[i];
        const id = this.viewport.warmAddPrim({
          texture: this.viewport.white, // unused for a textureName-d prim (channels resolve by name)
          textureName: sp.texName,
          outline: !sp.texName,         // food-chain F9: a textureless pawn part (the bunny) outlines
          flipX: sp.flipX,
          x: sp.x,
          y: sp.y,
          width: sp.w,
          height: sp.w,
          tint: sp.tint,
          geoColor: sp.geoColor,
          packed: i === 0 ? this.packedFor(kind) : undefined,
          seed: hash01(key),
          zIndex: sp.zIndex,
          hot: true, // a mover — its light/shadow participation is HOT-class only (pawn-render P2)
          rotation: facing, // P4: the record carries the TRUE cardinal (the frame follows it anyway)
          carrierOf: i > 0 ? parts[0].id : undefined, // P5: pieces name the carrier owner
          elevation: sp.elevation ?? 0, // z-positioning P3 — `y` is already shifted north by this
          light: lights[i], // trait-lights P4: part i carries light i (undefined past the set)
          // survival F1: the glyph rides the CARRIER slot only (a two-part pawn must
          // not print its letter twice).
          geoLabel: i === 0 ? this.geoLabels[kind - 1] : undefined,
        });
        parts.push({
          id,
          x: sp.x, y: sp.y, w: sp.w, texName: sp.texName, flipX: sp.flipX,
          tint: sp.tint, geoColor: sp.geoColor, zIndex: sp.zIndex,
        });
      }
      // The SPILL (F8): lights past the part count ride light-only sibling prims —
      // no texture, no outline, an EMPTY box (nothing bakes; the record path accepts
      // emit-only prims), anchored to the carrier so the reconciler re-derives their
      // ground position every frame. Mint once, remove with the pawn.
      const lightPrims: number[] = [];
      for (let j = specs.length; j < lights.length; j++) {
        lightPrims.push(this.viewport.warmAddPrim({
          texture: this.viewport.white,
          x: ax, y: ay, width: 0, height: 0,
          tint: 0,
          zIndex: orderRowBase,
          hot: true,
          rotation: facing,
          carrierOf: parts[0].id,
          light: lights[j],
        }));
      }
      if (m) {
        m.parts = parts;
        m.lightPrims = lightPrims;
        m.lightSig = lightSig;
        m.macroPosition = macroPosition;
      } else {
        this.movers.set(key, {
          parts, macroPosition, kind, def, authX: tileX, authY: tileY,
          spec: null,
          rx: tileX, ry: tileY, arx: tileX, ary: tileY, facing, // the chase starts AT the first authoritative tile
          lastFaceMs: 0,
          lastIntentTic: null,
          authTic: null,
          lightPrims,
          lightSig,
        });
      }
      return;
    }

    // Existing pawn: per slot, skip the warm re-bake when nothing that affects it changed.
    for (let i = 0; i < specs.length; i++) {
      const sp = specs[i];
      const pp = m.parts[i];
      if (
        pp.x === sp.x && pp.y === sp.y && pp.w === sp.w && pp.texName === sp.texName &&
        pp.flipX === sp.flipX && pp.tint === sp.tint && pp.geoColor === sp.geoColor &&
        pp.zIndex === sp.zIndex && m.macroPosition === macroPosition
      ) {
        continue;
      }
      const p = this.viewport.warmGetPrim(pp.id);
      if (p) {
        p.x = sp.x;
        p.y = sp.y;
        p.width = sp.w;
        p.height = sp.w;
        p.textureName = sp.texName;
        p.flipX = sp.flipX;
        p.tint = sp.tint;
        p.geoColor = sp.geoColor;
        p.zIndex = sp.zIndex;
        p.elevation = sp.elevation ?? 0; // z-positioning P3 — must track, or a moving elevated
                                         // part keeps the elevation it was CREATED with
        p.rotation = facing; // P4: keep the record's cardinal in step with the drawn facing
        this.viewport.warmRefreshPrim(pp.id);
        // hot-sync P1: the ONE hot dirty — the same eps crossing that re-bakes the sprite
        // raises the light/shadow/receiver dirt from the same snapshot.
        this.viewport.moverDirty(pp.id);
      }
      pp.x = sp.x; pp.y = sp.y; pp.w = sp.w; pp.texName = sp.texName; pp.flipX = sp.flipX;
      pp.tint = sp.tint; pp.geoColor = sp.geoColor; pp.zIndex = sp.zIndex;
    }
    m.macroPosition = macroPosition;
  }

  private remove(key: number): void {
    const m = this.movers.get(key);
    if (m) {
      for (const p of m.parts) this.viewport.warmRemovePrim(p.id);
      for (const id of m.lightPrims) this.viewport.warmRemovePrim(id);
      this.movers.delete(key);
    }
    this.pawnDefs.delete(key);
    this.pendingIntents.delete(key);
  }

  private onZoneClosed(macroPosition: number): void {
    for (const [key, m] of this.movers) {
      if (m.macroPosition === macroPosition) {
        for (const p of m.parts) this.viewport.warmRemovePrim(p.id);
        for (const id of m.lightPrims) this.viewport.warmRemovePrim(id);
        this.movers.delete(key);
        this.pawnDefs.delete(key);
        this.pendingIntents.delete(key);
      }
    }
  }
}
