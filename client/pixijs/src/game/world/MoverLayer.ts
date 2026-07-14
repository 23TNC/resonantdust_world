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
//! Unlike the old bespoke overlay-sprite path (a plain unlit sprite that couldn't key its own
//! background or take light), the pawn now lives in world-space warm RTs: added on first sight,
//! moved/turned in place as the pawn walks (a warm re-bake at higher-than-cold priority), and
//! dropped on `removed` or its zone closing. The viewport composites warm OVER cold and lights
//! the merged G-buffer, so the wolf lights + shadows like the world.

import { Texture } from "pixi.js";
import type { WasmClient, StateObject } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import type { PackedChannel } from "../viewport/material";
import { thingTexture } from "./WorldBridge";
import { placeThing, readLayout } from "./thingPlacement";

/** The reference variant drawn here. `objType` is the `reference_id` from
 *  `resonantdust_codec::refs` (0.2.3 reference model); `REF_HOT` (1) is any live/minted object
 *  — wolves + the player self-object. Game-type (pawn vs player) now lives in the payload `kind`,
 *  not the identity, so the mover layer draws every hot object and distinguishes by `objKind`. */
const REF_HOT = 1;

/** Base zIndex for warm pawn sprites — above the ground (tiles are `0`), matching cold things
 *  ({@link WorldBridge}'s `THING_Z_BASE`); the pawn's anchor tile-row is added so overlapping
 *  pawns/things paint front-over-back. */
const PAWN_Z_BASE = 1;

/** A well-distributed 32-bit hash → `[0, 1)` — a stable per-instance material seed from the
 *  pawn's `objectId`, so instances differ without the noise pattern swimming as the pawn walks. */
function hash01(a: number): number {
  a = Math.imul(a ^ (a >>> 16), 0x45d9f3b);
  a = Math.imul(a ^ (a >>> 16), 0x45d9f3b);
  return ((a ^ (a >>> 16)) >>> 0) / 4294967296;
}

/** One live pawn: its warm prim id + zone (so a zone close drops it) and the last-synced spec
 *  (so an unchanged state update skips the warm re-bake). */
interface Mover {
  id: number;
  zoneId: number;
  x: number;
  y: number;
  texName: string | undefined;
  cell: number | undefined;
  flipX: boolean;
  tint: number;
  geoColor: number;
  zIndex: number;
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

  constructor(
    private readonly client: WasmClient,
    private content: Content,
    private readonly viewport: Viewport,
  ) {
    this.refreshTables();
    this.unsubs.push(client.onStateObject((obj) => this.onStateObject(obj)));
    // A zone leaving the subscription sends no per-entity delete, so drop its movers.
    this.unsubs.push(client.onZoneClosed((zoneId) => this.onZoneClosed(zoneId)));
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

  // ── internals ───────────────────────────────────────────────────────

  private refreshTables(): void {
    this.thingStems = this.content.thingTextureStems();
    this.thingLayout = this.content.thingLayout();
    this.thingPacked = this.content.thingPackedChannels();
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

  private onStateObject(obj: StateObject): void {
    if (obj.objType !== REF_HOT) return; // hot movers only (wolves + player self-object)
    const key = obj.objectId;
    if (obj.removed) {
      this.remove(key);
      return;
    }
    // World TILE + tint/geoColor for the pawn's cell (stem/layout/packed come from the content
    // tables, keyed by objKind). Placed via the shared def layout exactly like cold things, so
    // the z-sort by anchor row makes overlapping pawns/things paint front-over-back and a tall
    // sprite rises past its cell.
    const prim = this.content.moverPrim(obj.zoneId, obj.location, obj.objKind);
    const tileX = prim[0];
    const tileY = prim[1];
    const tint = prim[2];
    const geoColor = prim[3];

    // Facing (+ west flip) from the entity's rotation; variant is a stable per-entity pick.
    const stem = this.thingStems[obj.objKind - 1];
    const tex = thingTexture(stem, obj.rotation, obj.objectId, /* mover */ true);
    const box = placeThing(tileX, tileY, readLayout(this.thingLayout, obj.objKind), tex.flipX);
    const { x, y } = box;
    const size = box.width; // square box; used for warm prim width/height
    const zIndex = PAWN_Z_BASE + box.zRow;

    let m = this.movers.get(key);
    if (!m) {
      const id = this.viewport.warmAddPrim({
        texture: Texture.EMPTY, // fallback texture is unused for a textureName'd prim (channels resolve by name)
        textureName: tex.name,
        flipX: tex.flipX,
        cell: tex.cell,
        x,
        y,
        width: size,
        height: size,
        tint,
        geoColor,
        packed: this.packedFor(obj.objKind),
        seed: hash01(obj.objectId),
        zIndex,
      });
      this.movers.set(key, {
        id, zoneId: obj.zoneId, x, y, texName: tex.name, cell: tex.cell, flipX: tex.flipX, tint, geoColor, zIndex,
      });
      return;
    }

    // Existing pawn: skip the warm re-bake when nothing that affects the bake changed.
    if (
      m.x === x && m.y === y && m.texName === tex.name && m.cell === tex.cell &&
      m.flipX === tex.flipX && m.tint === tint && m.geoColor === geoColor && m.zoneId === obj.zoneId
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
    m.zoneId = obj.zoneId;
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

  private onZoneClosed(zoneId: number): void {
    for (const [key, m] of this.movers) {
      if (m.zoneId === zoneId) {
        this.viewport.warmRemovePrim(m.id);
        this.movers.delete(key);
      }
    }
  }
}
