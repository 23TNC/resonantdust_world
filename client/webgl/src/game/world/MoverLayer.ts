//! The mover layer — the tick pipeline's mobile entities (pawns) fed into the viewport's WARM
//! cache as ordinary primitives.
//!
//! A pawn (a wolf) is a WARM prim: a bottom-anchored sprite whose texture stem comes from the
//! content bundle (`thing_texture_stems[objKind-1]`, e.g. `pawn/animal/wolf/default`), its facing
//! picked from the entity's `rotation` via {@link thingTexture} (the SAME rotation→texture+flip
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
import { thingTexture } from "./WorldBridge";
import { placeThing, readLayout } from "./thingPlacement";

/** Base zIndex for warm pawn sprites — above the ground (tiles are `0`), matching cold things
 *  ({@link WorldBridge}'s `THING_Z_BASE`); the pawn's anchor tile-row is added so overlapping
 *  pawns/things paint front-over-back. */
const PAWN_Z_BASE = 1;

/** Speculation applies a new position only past this tile delta — keeps the warm re-bake
 *  cadence proportional to actual motion, not the frame rate. */
const SPEC_APPLY_EPS = 1 / 32;

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

/** One live pawn: its warm prim id + zone (so a zone close drops it), the last-synced spec
 *  (so an unchanged update skips the warm re-bake), the last AUTHORITATIVE tile (speculation's
 *  seed), and the live speculation, if any. */
interface Mover {
  id: number;
  macroPosition: number;
  kind: number;
  authX: number;
  authY: number;
  x: number;
  y: number;
  texName: string | undefined;
  cell: number | undefined;
  flipX: boolean;
  tint: number;
  geoColor: number;
  zIndex: number;
  spec: Spec | null;
  /** The freshest intent tic ever armed — dedups replayed/duplicate `event` deliveries even
   *  after the spec cleared (zone re-subscribes replay history — first-pawns I2). */
  lastIntentTic: number | null;
}

export class MoverLayer {
  private readonly movers = new Map<number, Mover>();
  private readonly unsubs: Array<() => void> = [];
  /** Per-kind texture-stem / size / packed-channel tables from the content bundle, indexed by
   *  `objKind - 1` (same tables + indexing WorldBridge uses for cold things). Refreshed on
   *  hot-swap. */
  private thingStems: string[] = [];
  private thingLayout: Float64Array = new Float64Array();
  private thingPacked: Float64Array = new Float64Array();
  /** Per-kind authored tics-per-tile (`0` = unauthored → {@link defaultTicsPerTile}). */
  private thingSpeed: Float64Array = new Float64Array();

  constructor(
    private readonly client: WasmClient,
    private content: Content,
    private readonly viewport: Viewport,
  ) {
    this.refreshTables();
    this.unsubs.push(client.onStateObject((obj) => this.onStateObject(obj)));
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
    for (const m of this.movers.values()) this.viewport.warmRemovePrim(m.id);
    this.movers.clear();
  }

  /** Per-frame: advance every live speculation along its greedy line at the tic estimate. */
  tick(): void {
    for (const [key, m] of this.movers) {
      const s = m.spec;
      if (!s) continue;
      const d = this.client.ticDelta(s.eventTic);
      if (d === null || d < 0) continue; // the move is still in the estimated future
      const p = walkGreedy(s.fromX, s.fromY, s.destX, s.destY, d / s.ticsPerTile);
      if (Math.abs(p.x - s.appliedX) < SPEC_APPLY_EPS && Math.abs(p.y - s.appliedY) < SPEC_APPLY_EPS) {
        continue;
      }
      s.appliedX = p.x;
      s.appliedY = p.y;
      this.applyVisual(m, key, m.kind, p.x, p.y, p.facing, m.macroPosition);
      // Arrived speculatively — hold at dest; the authoritative final State clears the spec.
    }
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
   *  authoritative tile. An intent for a pawn we haven't seen yet is dropped — its seed `State`
   *  arrives on the same settle and reseeds everything. */
  private onMoveIntent(intent: MoveIntent): void {
    const m = this.movers.get(intent.entityReference);
    if (!m) return;
    const tpt = this.speedFor(m.kind);
    // The `event` table replays HISTORY on subscribe (no retention yet — first-pawns I2), and
    // deliveries can arrive out of order — so guard: no clock ⇒ can't speculate; an intent whose
    // move must already be over ⇒ the authoritative rows carry the outcome; an intent serially
    // older than the live spec ⇒ superseded. Only a fresh, live intent arms speculation.
    const d = this.client.ticDelta(intent.eventTic);
    if (d === null) return;
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
    // Content kind → sprite. A legacy-placed pawn carries `definitionReference` 0 (`PLACE` has
    // no def operand), so fall back to the first thing kind; `CREATE`-minted pawns carry the
    // real kind and render their own sprite.
    const kind =
      obj.definitionReference >= 1 && obj.definitionReference <= this.thingStems.length
        ? obj.definitionReference
        : 1;

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
      m.authX = obj.tileX;
      m.authY = obj.tileY;
      m.kind = kind;
    }
    this.applyVisual(m ?? null, key, kind, obj.tileX, obj.tileY, obj.facing, obj.macroPosition);
  }

  /** Create or mutate the pawn's warm prim at (possibly fractional) global tile `(tileX, tileY)`
   *  with `facing`. The shared path for authoritative rows AND per-frame speculation. */
  private applyVisual(
    m: Mover | null,
    key: number,
    kind: number,
    tileX: number,
    tileY: number,
    facing: number,
    macroPosition: number,
  ): void {
    const prim = this.content.moverPrim(macroPosition, 0, kind);
    const tint = prim[2];
    const geoColor = prim[3];

    // Facing (+ west flip) from the entity's facing; variant is a stable per-entity pick.
    const stem = this.thingStems[kind - 1];
    const tex = thingTexture(stem, facing, key, /* mover */ true);
    const box = placeThing(tileX, tileY, readLayout(this.thingLayout, kind), tex.flipX, !tex.name);
    const { x, y } = box;
    const size = box.width; // square box; used for warm prim width/height
    const zIndex = PAWN_Z_BASE + box.zRow;

    if (!m) {
      const id = this.viewport.warmAddPrim({
        texture: this.viewport.white, // fallback texture is unused for a textureName-d prim (channels resolve by name)
        textureName: tex.name,
        flipX: tex.flipX,
        cell: tex.cell,
        x,
        y,
        width: size,
        height: size,
        tint,
        geoColor,
        packed: this.packedFor(kind),
        seed: hash01(key),
        zIndex,
      });
      this.movers.set(key, {
        id, macroPosition, kind, authX: tileX, authY: tileY,
        x, y, texName: tex.name, cell: tex.cell, flipX: tex.flipX, tint, geoColor, zIndex,
        spec: null,
        lastIntentTic: null,
      });
      return;
    }

    // Existing pawn: skip the warm re-bake when nothing that affects the bake changed.
    if (
      m.x === x && m.y === y && m.texName === tex.name && m.cell === tex.cell &&
      m.flipX === tex.flipX && m.tint === tint && m.geoColor === geoColor && m.macroPosition === macroPosition
    ) {
      return;
    }
    const p = this.viewport.warmGetPrim(m.id);
    if (p) {
      p.x = x;
      p.y = y;
      p.textureName = tex.name;
      p.cell = tex.cell;
      p.flipX = tex.flipX;
      p.tint = tint;
      p.geoColor = geoColor;
      p.zIndex = zIndex;
      this.viewport.warmRefreshPrim(m.id);
    }
    m.macroPosition = macroPosition;
    m.x = x; m.y = y; m.texName = tex.name; m.cell = tex.cell; m.flipX = tex.flipX;
    m.tint = tint; m.geoColor = geoColor; m.zIndex = zIndex;
  }

  private remove(key: number): void {
    const m = this.movers.get(key);
    if (m) {
      this.viewport.warmRemovePrim(m.id);
      this.movers.delete(key);
    }
  }

  private onZoneClosed(macroPosition: number): void {
    for (const [key, m] of this.movers) {
      if (m.macroPosition === macroPosition) {
        this.viewport.warmRemovePrim(m.id);
        this.movers.delete(key);
      }
    }
  }
}
