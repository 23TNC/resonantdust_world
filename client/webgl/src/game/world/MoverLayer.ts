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
//! (greedy straight line, e/w-first facing) at the kind's authored tics-per-tile (the bundle's
//! `thingSpeed` table — pawn-movement F1/F5: the SAME value the worker spaces hops with),
//! driven by the client's wall↔tic estimate — so the wolf GLIDES between tiles with zero
//! per-hop bandwidth.
//! Authoritative `State` rows snap/reseed the speculation and log the observed error (F8 —
//! the data the re-anchor cadence will be tuned on); `ZoneClosed` drops it.

import type { Texture } from "../../gl";
import type { WasmClient, StateObject, MoveIntent } from "../../client/WasmClient";
import { defaultTicsPerTile } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import type { PackedChannel } from "../viewport/material";
import { moverSlotTexture } from "./WorldBridge";
import { placeThing, readLayout } from "./thingPlacement";

/** Base zIndex for warm pawn sprites — above the ground (tiles are `0`), matching cold things
 *  ({@link WorldBridge}'s `THING_Z_BASE`); the pawn's anchor tile-row is added so overlapping
 *  pawns/things paint front-over-back. */
const PAWN_Z_BASE = 1;

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
  offsetY: number;
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
}

/** Speculation applies a new position only past this tile delta — keeps the warm re-bake
 *  cadence proportional to actual motion, not the frame rate. */
const SPEC_APPLY_EPS = 1 / 32;

/** How much of a z-ROW one unit of authored slot `depth` is worth (F1). Slot ordering must stay
 *  strictly INSIDE the pawn's row so the row keeps deciding which pawn is in front: with the
 *  per-slot tiebreak below, `|depth| < 45` is safely within `(-0.5, +0.5)`. */
const SLOT_DEPTH_Z = 0.01;
/** A stable within-depth tiebreak so two slots at the same depth keep their authored order. */
const SLOT_ORDER_Z = 0.0001;

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

/** A live movement speculation: walk from the last authoritative tile toward the intent's dest,
 *  `progress = ticDelta(eventTic) / ticsPerTile` steps along the greedy line. */
interface Spec {
  fromX: number;
  fromY: number;
  destX: number;
  destY: number;
  /** The tic `fromX/fromY` was authoritative at — progress measures from here. */
  eventTic: number;
  ticsPerTile: number;
  /** Last applied fractional tile, to skip sub-epsilon re-bakes. */
  appliedX: number;
  appliedY: number;
}

/** The server's stepping rule, mirrored EXACTLY (worker `apply` MOVE_TO): `p` fractional greedy
 *  steps from `(fx,fy)` toward `(dx,dy)`; facing from the active step, e/w winning diagonals. */
function walkGreedy(
  fx: number, fy: number, dx: number, dy: number, p: number,
): { x: number; y: number; facing: number; done: boolean } {
  let cx = fx;
  let cy = fy;
  let facing = dx > cx ? 1 : dx < cx ? 3 : dy > cy ? 0 : 2;
  let remaining = p;
  for (;;) {
    const sx = Math.sign(dx - cx);
    const sy = Math.sign(dy - cy);
    if (sx === 0 && sy === 0) return { x: cx, y: cy, facing, done: true };
    facing = sx > 0 ? 1 : sx < 0 ? 3 : sy > 0 ? 0 : 2;
    if (remaining < 1) {
      return { x: cx + sx * remaining, y: cy + sy * remaining, facing, done: false };
    }
    cx += sx;
    cy += sy;
    remaining -= 1;
  }
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
  spec: Spec | null;
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
  /** Facing last applied to the prim (the TARGET's facing applies immediately — the pawn
   *  turns toward its path at the seed, then walks into it). */
  facing: number;
  /** The freshest intent tic ever armed — dedups replayed/duplicate `event` deliveries even
   *  after the spec cleared (zone re-subscribes replay history — first-pawns I2). */
  lastIntentTic: number | null;
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

/** Serial u16 comparison: is `a` strictly newer than `b` on the wrapping tic ring? */
function ticNewer(a: number, b: number): boolean {
  return ((((a - b) & 0xffff) << 16) >> 16) > 0;
}

export class MoverLayer {
  private readonly movers = new Map<number, Mover>();
  /** Intents that arrived before their pawn's first `State` or before the tic clock anchored
   *  — HELD, never dropped (pawn-movement I1: a dropped live intent degrades the whole trip
   *  to its two authoritative snaps). {@link tick} re-evaluates them once both preconditions
   *  hold; the stale/dedup guards then arm or discard. Freshest per entity wins. */
  private readonly pendingIntents = new Map<number, MoveIntent>();
  private readonly unsubs: Array<() => void> = [];
  /** Per-kind texture-stem / size / packed-channel tables from the content bundle, indexed by
   *  `objKind - 1` (same tables + indexing WorldBridge uses for cold things). Refreshed on
   *  hot-swap. */
  private thingStems: string[] = [];
  private thingLayout: Float64Array = new Float64Array();
  private thingPacked: Float64Array = new Float64Array();
  /** Per-kind authored tics-per-tile (`0` = unauthored → {@link defaultTicsPerTile}). */
  private thingSpeed: Float64Array = new Float64Array();

  /** Payload part slots per entity (`slot → def`), joined from `PawnParts` events — either
   *  side of the join may arrive first (human-pawns P3). */
  private readonly pawnDefs = new Map<number, Map<number, number>>();

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
    this.refreshTables();
    this.unsubs.push(client.onStateObject((obj) => this.onStateObject(obj)));
    this.unsubs.push(client.onPawnParts((p) => this.onPawnParts(p)));
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
      const s = m.spec;
      if (s) {
        const d = this.client.ticDelta(s.eventTic);
        if (d !== null && d > 0) {
          const p = walkGreedy(s.fromX, s.fromY, s.destX, s.destY, d / s.ticsPerTile);
          tx = p.x;
          ty = p.y;
          facing = p.facing;
          s.appliedX = p.x; // spec-space bookkeeping — landing error still measures the SPEC
          s.appliedY = p.y;
        }
      }
      // Chase: close the render→target gap at ≤ CHASE_CAP × true speed, leaning harder the
      // further behind (CHASE_GAIN), snapping past CHASE_SNAP_TILES. Chebyshev metric and
      // per-axis stepping match the greedy path's diagonal geometry.
      const gx = tx - m.rx;
      const gy = ty - m.ry;
      const gap = Math.max(Math.abs(gx), Math.abs(gy));
      if (gap > 0) {
        const tilesPerSec = this.client.ticsPerSec() / this.speedFor(m.kind);
        const step = gap > CHASE_SNAP_TILES
          ? gap // hopeless — snap
          : tilesPerSec * Math.min(CHASE_CAP, 1 + gap * CHASE_GAIN) * dtSec;
        // The chase STATE always advances (sub-eps steps accumulate); only the warm
        // re-bake is eps-gated, against the last APPLIED position.
        m.rx += Math.sign(gx) * Math.min(step, Math.abs(gx));
        m.ry += Math.sign(gy) * Math.min(step, Math.abs(gy));
      }
      if (
        Math.abs(m.rx - m.arx) >= SPEC_APPLY_EPS || Math.abs(m.ry - m.ary) >= SPEC_APPLY_EPS ||
        facing !== m.facing
      ) {
        m.arx = m.rx;
        m.ary = m.ry;
        m.facing = facing;
        this.applyVisual(m, key, m.kind, m.def, m.rx, m.ry, facing, m.macroPosition);
      }
    }
  }

  /** ui-select P1: the warm prim id for a pawn entity — the CARRIER (slot 0) prim; outline/
   *  details follow it. */
  primIdOf(entity: number): number | null {
    return this.movers.get(entity)?.parts[0]?.id ?? null;
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
      tileX: m.authX, tileY: m.authY, facing: m.facing,
      macroPosition: m.macroPosition, moving: !!m.spec, ticsPerTile: this.speedFor(m.kind),
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
   *  (either side may arrive first; a pre-mover payload just waits in the map). */
  private onPawnParts(p: { entityReference: number; parts: { slot: number; def: number }[] }): void {
    const map = new Map<number, number>();
    for (const e of p.parts) map.set(e.slot, e.def);
    this.pawnDefs.set(p.entityReference, map);
    const m = this.movers.get(p.entityReference);
    if (m) this.applyVisual(m, p.entityReference, m.kind, m.def, m.rx, m.ry, m.facing, m.macroPosition);
  }

  // ── internals ───────────────────────────────────────────────────────

  private refreshTables(): void {
    this.thingStems = this.content.thingTextureStems();
    this.thingLayout = this.content.thingLayout();
    this.thingPacked = this.content.thingPackedChannels();
    this.thingSpeed = this.content.thingSpeed();
  }

  /** The kind's tics-per-tile — the speculation rate, from the content bundle so it matches
   *  the worker's continuation spacing exactly (one speed authority, keyed by kind). */
  private speedFor(kind: number): number {
    const s = this.thingSpeed[kind - 1];
    return s > 0 ? s : defaultTicsPerTile();
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
    const tpt = this.speedFor(m.kind);
    // The `event` table replays HISTORY on subscribe (no retention yet — first-pawns I2), and
    // deliveries can arrive out of order — so guard: an intent whose move must already be over
    // ⇒ the authoritative rows carry the outcome; an intent serially older than the live spec
    // ⇒ superseded. Only a fresh, live intent arms speculation.
    const span = Math.max(Math.abs(intent.tileX - m.authX), Math.abs(intent.tileY - m.authY));
    if (d > (span + 2) * tpt) return; // finished long ago — stale replay
    if (m.lastIntentTic !== null && ((((intent.eventTic - m.lastIntentTic) & 0xffff) << 16) >> 16) <= 0) {
      return; // a duplicate delivery or a serially older intent — superseded
    }
    m.lastIntentTic = intent.eventTic;
    console.debug(
      `[mover] intent armed entity=${intent.entityReference.toString(16)} dest=(${intent.tileX},${intent.tileY}) tic=${intent.eventTic} d=${d.toFixed(1)}`,
    );
    m.spec = {
      fromX: m.authX,
      fromY: m.authY,
      destX: intent.tileX,
      destY: intent.tileY,
      eventTic: intent.eventTic,
      ticsPerTile: tpt,
      appliedX: m.authX,
      appliedY: m.authY,
    };
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

    // F8 — the authoritative row corrects speculation: log the observed error (the data the
    // re-anchor cadence is tuned on), then snap. The final tile clears the spec; an interim
    // authoritative resolve (another event touched the pawn) reseeds it.
    if (m?.spec) {
      const s = m.spec;
      const err = Math.max(Math.abs(s.appliedX - obj.tileX), Math.abs(s.appliedY - obj.tileY));
      if (obj.tileX === s.destX && obj.tileY === s.destY) {
        console.debug(
          `[mover] spec landed e=${err.toFixed(2)} tiles entity=${key.toString(16)} tic=${obj.tic}`,
        );
        m.spec = null;
      } else {
        console.debug(
          `[mover] spec reseed e=${err.toFixed(2)} tiles entity=${key.toString(16)} tic=${obj.tic}`,
        );
        s.fromX = obj.tileX;
        s.fromY = obj.tileY;
        s.eventTic = obj.tic;
        s.appliedX = obj.tileX;
        s.appliedY = obj.tileY;
      }
    }

    if (m) {
      // Existing mover: the authoritative row STEERS (auth tile, zone, kind); the render
      // keeps chasing from wherever it is (F6 — no direct snap; `tick` closes the gap at
      // the capped rate, or snaps itself past the hopeless threshold).
      m.authX = obj.tileX;
      m.authY = obj.tileY;
      m.kind = kind;
      m.def = obj.definitionReference;
      m.macroPosition = obj.macroPosition;
      return;
    }
    this.applyVisual(null, key, kind, obj.definitionReference, obj.tileX, obj.tileY, obj.facing, obj.macroPosition);
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

    // Slot 0 — the carrier box from the kind's layout (the wolf's whole visual).
    const d0 = defOf(0);
    const layout = readLayout(this.thingLayout, kind);
    const tex0 = moverSlotTexture(stemOf(slots[0], d0), facing, variantOf(d0), slots[0].part, has);
    scaleSlot(slots[0], tex0.name);
    const box = placeThing(tileX, tileY, layout, tex0.flipX, !tex0.name);
    const size0 = box.width; // square box
    // The pawn's ROW band. Slots sort WITHIN it by their own facing-resolved depth (F1) — the
    // row still decides which pawn is in front, and the blit's warm-over-cold compare reads the
    // row out of `zdepth_world` (from `prim.y + height`), which no slot z touches.
    const zRowBase = PAWN_Z_BASE + box.zRow;
    const slotZ = (s: MoverPart, i: number): number =>
      zRowBase + facingDepth(s.depth, facing) * SLOT_DEPTH_Z + i * SLOT_ORDER_Z;
    // The carrier's game anchor (base-centre) + the px-per-tile scale for slot offsets. Every box
    // is `span × SQUARE` now (F4 — the scale lives in the art), so one tile is `size0 / span`.
    const ax = box.x + size0 * 0.5;
    const ay = box.y + size0;
    const tilePx = layout.span > 0 ? size0 / layout.span : size0;

    interface SlotSpec {
      texName: string | undefined; flipX: boolean; x: number; y: number; w: number;
      tint: number; geoColor: number; zIndex: number;
    }
    const specs: SlotSpec[] = [
      { texName: tex0.name, flipX: tex0.flipX, x: box.x, y: box.y, w: size0,
        tint: slots[0].tint, geoColor: slots[0].geoColor, zIndex: slotZ(slots[0], 0) },
    ];
    for (let i = 1; i < slots.length; i++) {
      const s = slots[i];
      const di = defOf(i);
      const tex = moverSlotTexture(stemOf(s, di), facing, variantOf(di), s.part, has);
      scaleSlot(s, tex.name);
      // The slot's OWN frame span in world px — never slot 0's box times a factor (F4): the drawn
      // box must equal the def's pow2 frame span or the lighting card mis-sizes by the ratio.
      const w = (s.span > 0 ? s.span : 1) * tilePx;
      // The slot's authored offset (tiles) from the carrier anchor; x mirrors with a west
      // facing so the part stays on the sprite's correct side.
      const ox = (tex.flipX ? -s.offsetX : s.offsetX) * tilePx;
      const cx = ax + ox;
      const cy = ay + s.offsetY * tilePx;
      specs.push({
        texName: tex.name, flipX: tex.flipX, x: cx - w * 0.5, y: cy - w * 0.5, w,
        tint: s.tint, geoColor: s.geoColor,
        zIndex: slotZ(s, i),
      });
    }

    // A slot-count change (a late payload join won't change the count — the DSL fixes it —
    // but a content hot-swap can): rebuild from scratch.
    if (m && m.parts.length !== specs.length) {
      for (const p of m.parts) this.viewport.warmRemovePrim(p.id);
      m.parts = [];
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
          layer: i,
        });
        parts.push({
          id,
          x: sp.x, y: sp.y, w: sp.w, texName: sp.texName, flipX: sp.flipX,
          tint: sp.tint, geoColor: sp.geoColor, zIndex: sp.zIndex,
        });
      }
      if (m) {
        m.parts = parts;
        m.macroPosition = macroPosition;
      } else {
        this.movers.set(key, {
          parts, macroPosition, kind, def, authX: tileX, authY: tileY,
          spec: null,
          rx: tileX, ry: tileY, arx: tileX, ary: tileY, facing, // the chase starts AT the first authoritative tile
          lastIntentTic: null,
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
      this.movers.delete(key);
    }
    this.pawnDefs.delete(key);
    this.pendingIntents.delete(key);
  }

  private onZoneClosed(macroPosition: number): void {
    for (const [key, m] of this.movers) {
      if (m.macroPosition === macroPosition) {
        for (const p of m.parts) this.viewport.warmRemovePrim(p.id);
        this.movers.delete(key);
        this.pawnDefs.delete(key);
        this.pendingIntents.delete(key);
      }
    }
  }
}
