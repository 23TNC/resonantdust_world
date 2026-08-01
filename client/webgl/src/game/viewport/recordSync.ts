//! The record RECONCILER (lighting-correctness P1b) — the seam I1 pinned as missing.
//!
//! The rework proved the record layer on hand-built snapshots (`buildRecords`, one-shot, racing
//! frame streaming, movers absent, content lights unread). This replaces that with a per-frame
//! reconcile of the LIVE scene: every standing prim of BOTH caches (cold things + warm movers)
//! keeps a prim record; every stem keeps a definition (re-written when the resolver's frames
//! move); every `Primitive.light` (the DSL-authored torch struct) makes its prim an EMITTER with
//! its AUTHORED reach in the stored lane (P1). Presence and the light map rebuild from the same
//! walk, so the record set and the drawn set cannot describe different scenes.
//!
//! Cost stance: ~700 prims/frame of compare-writes is the same shape the old `buildCasters` ran
//! per frame; `Records` compare-writes mean an unchanged scene uploads nothing. The presence map
//! rewrites only occupied tiles (+ clears newly-vacated ones); the light map clears + rebuilds
//! only when an emitter's (index, tile, reach) signature changes.

import type { Primitive } from "./SquareCache";
import type { Texture } from "../../gl";
import type { TextureResolver } from "../../textures";
import { Records, INDEX_NONE, INTENSITY_MAX, REACH_MAX_TILES } from "./records";
import { SQUARE, UNITS_PER_TILE } from "./squareMath";
import { drawnForWorldHeight } from "./worldTilt";

const U = SQUARE / UNITS_PER_TILE; // world px per unit

/** DSL light intensity (1.0 = a normal torch, 4.0 = the overbright ceiling) → the u6 lane. */
function intensityLane(intensity: number): number {
  return Math.max(0, Math.min(INTENSITY_MAX, Math.round((intensity * INTENSITY_MAX) / 4)));
}

export class RecordSync {
  /** `${stem}#${cell}` → definition block. */
  private readonly defByKey = new Map<string, number>();
  /** `c${id}` / `w${id}` (cache prim) → record index. */
  private readonly primByCache = new Map<string, number>();
  /** World tiles the presence map currently describes (packed `(tx+2048)<<16 | (ty+2048)`). */
  private occupied = new Set<number>();
  /** The emitter set the light map currently describes. */
  private lightSig = "";
  /** Resolver frames moved (a lod landed / a page repacked) → re-write every definition. */
  private defsStale = true;
  /** Atlas PAGE registry, first-seen order — a def's page index rides its anchor.x lane
   *  (P3: silhouettes sample the page they live on; two pages bind, more fall back). */
  private readonly pages: Texture[] = [];

  constructor(private readonly rec: Records) {}

  markDefsStale(): void {
    this.defsStale = true;
  }

  /** One rotation px's fields from a resolved stem: span from the MANIFEST (I2 — never atlas
   *  px), subframe from the sprite's own alpha (post pre-atlas scale), and the frame's atlas
   *  scale carried in the def's otherwise-meaningless SEED lane as `pxPerUnit × 8` — the
   *  silhouette sampler needs it, and hardcoding 8 was only ever true at the 128-px lod (P3). */
  private defFields(resolver: TextureResolver, stem: string, cell: number, boxSpanHint: number):
      { frameX: number; frameY: number; frameSpan: number; anchorX: number; anchorY: number;
        subX: number; subY: number; subW: number; subH: number; seed: number;
        castType: number; receiveType: number } | null {
    const frame = resolver.resolve(stem, "albedo", cell)?.frame;
    if (!frame) return null;
    let page = this.pages.indexOf(frame.source);
    if (page < 0) { this.pages.push(frame.source); page = this.pages.length - 1; }
    const spanTiles = Math.max(1, Math.min(16, Math.round(resolver.spanOf(stem) ?? boxSpanHint)));
    const pxPerUnit = frame.w / (spanTiles * UNITS_PER_TILE);
    const bb = resolver.opaqueBBox(stem);
    const fu = spanTiles * UNITS_PER_TILE;
    const subX = bb ? Math.round(bb.fx * fu) : 0;
    const subY = bb ? Math.round(bb.fy * fu) : 0;
    const subW = Math.min(fu - subX, Math.max(1, bb ? Math.round(bb.fw * fu) : fu));
    const subH = Math.min(fu - subY, Math.max(1, bb ? Math.round(bb.fh * fu) : fu));
    return {
      frameX: Math.round(frame.x / pxPerUnit),
      frameY: Math.round(frame.y / pxPerUnit),
      frameSpan: spanTiles,
      anchorX: Math.min(3, page), anchorY: 2, // anchor.x = the ATLAS PAGE (P3); y unused
      subX, subY, subW, subH,
      seed: Math.max(1, Math.min(255, Math.round(pxPerUnit * 8))),
      castType: 1, receiveType: 2,
    };
  }

  /** A COLD thing's degenerate block: one facing, rotations 0-2 identical, px 3 mirrored
   *  (P2 — the west draw). Returns 0 while the stem has no streamed frame. */
  private defFor(resolver: TextureResolver, stem: string, cell: number, boxSpanHint: number): number {
    const key = `${stem}#${cell}`;
    let block = this.defByKey.get(key);
    if (block !== undefined && !this.defsStale) return block;
    const fields = this.defFields(resolver, stem, cell, boxSpanHint);
    if (!fields) return block ?? 0;
    if (block === undefined) {
      block = this.rec.allocDefinition(4);
      this.defByKey.set(key, block);
    }
    // P3 superseded P2's data-side mirror: rotation 3 keeps the PLAIN east fields — the
    // occlusion shader mirrors placement AND sample about the frame centre (the atlas art
    // itself is never flipped, so a mirrored subX pointed the sampler OFF the art).
    for (let r = 0; r < 4; r++) this.rec.writeDefinition(block, r, fields);
    return block;
  }

  /** A MOVER's kind-level block (P3 — `base + rotation` made REAL): rotations are FACINGS,
   *  r0 = the south frame, r1 = east, r2 = north, r3 = east MIRRORED (west). This is what lets
   *  `cast_type 2` fetch the SIDE frame (r1) for the perpendicular n/s caster card while the
   *  receiver path reads the prim's own facing — one block, no special case. Falls back to the
   *  degenerate per-stem block until all three facings have streamed. */
  private moverDefFor(resolver: TextureResolver, stem: string, cell: number, boxSpanHint: number): { block: number; kind: boolean } {
    // Split `<base>/<facing>[.<part>]` — the same grammar moverSlotTexture writes.
    const cut = stem.lastIndexOf("/");
    const seg = stem.slice(cut + 1);
    const dot = seg.indexOf(".");
    const facing = dot >= 0 ? seg.slice(0, dot) : seg;
    const part = dot >= 0 ? seg.slice(dot) : "";
    if (facing !== "s" && facing !== "e" && facing !== "n") {
      return { block: this.defFor(resolver, stem, cell, boxSpanHint), kind: false };
    }
    const base = stem.slice(0, cut);
    const key = `${base}${part}#kind#${cell}`;
    let block = this.defByKey.get(key);
    if (block !== undefined && !this.defsStale) return { block, kind: true };
    const s = this.defFields(resolver, `${base}/s${part}`, cell, boxSpanHint);
    const e = this.defFields(resolver, `${base}/e${part}`, cell, boxSpanHint);
    const n = this.defFields(resolver, `${base}/n${part}`, cell, boxSpanHint);
    if (!s || !e || !n) {
      // Not all facings streamed — the degenerate single-facing block keeps the mover lit.
      return { block: this.defFor(resolver, stem, cell, boxSpanHint), kind: false };
    }
    if (block === undefined) {
      block = this.rec.allocDefinition(4);
      this.defByKey.set(key, block);
    }
    this.rec.writeDefinition(block, 0, s);
    this.rec.writeDefinition(block, 1, e);
    this.rec.writeDefinition(block, 2, n);
    this.rec.writeDefinition(block, 3, e);   // west = the east frame; the shader mirrors (P3)
    return { block, kind: true };
  }

  /** One reconcile of the live scene. Call per frame while lit; cheap when nothing changed. */
  sync(gl: WebGL2RenderingContext, resolver: TextureResolver | null,
       cold: Iterable<Primitive>, warm: Iterable<Primitive>): void {
    const rec = this.rec;
    const live = new Set<string>();
    const byTile = new Map<number, { index: number; layer: number }[]>();
    const emitters: { index: number; tileX: number; tileY: number; reach: number }[] = [];

    // z-positioning P2b: a carried piece STANDS WHERE ITS CARRIER STANDS. Its own opaque bbox is
    // no guide — a head's art bottom is the bottom of the head, not the pawn's feet, which left
    // head and body 4 units apart even with the elevation added back. So the carrier's ground row
    // is resolved FIRST and the pieces adopt it; the vertical gap between them then IS the piece's
    // elevation, derived rather than authored, and head and body align by construction.
    const coldA = [...cold], warmA = [...warm];
    const groundRowOf = (p: Primitive): number => {
      const bb = p.textureName && resolver ? resolver.opaqueBBox(p.textureName) : null;
      return p.y + (bb ? (bb.fy + bb.fh) * p.height : p.height);
    };
    const carrierGround = new Map<number, { ax: number; ay: number }>();
    for (const p of warmA) {
      if (p.carrierOf === undefined) {
        carrierGround.set(p.id, { ax: p.x + p.width / 2, ay: groundRowOf(p) });
      }
    }

    for (const [prefix, prims] of [["c", coldA], ["w", warmA]] as const) {
      for (const p of prims) {
        const L = p.light;
        if (!p.textureName && !L) continue;      // a flat tint rect neither casts nor emits
        const key = prefix + p.id;
        live.add(key);
        let idx = this.primByCache.get(key);
        if (idx === undefined) {
          idx = rec.allocPrim();
          this.primByCache.set(key, idx);
        }
        const boxSpanHint = Math.max(1, Math.round(p.width / SQUARE));
        // A MOVER (carries its true cardinal) gets the kind-level rotation block; a cold thing
        // the degenerate per-stem one. P3: n/s facings cast from the PERPENDICULAR card
        // (cast_type 2) whose silhouette is the block's r1 (side) frame.
        const isMover = p.rotation !== undefined;
        let def = 0, rotation = 0, castType = 0;
        if (p.textureName && resolver) {
          if (isMover) {
            const r = this.moverDefFor(resolver, p.textureName, p.cell ?? 0, boxSpanHint);
            def = r.block;
            rotation = r.kind ? (p.rotation ?? 0) & 3 : (p.flipX ? 3 : 0);
            castType = def === INDEX_NONE ? 0 : r.kind && (rotation === 0 || rotation === 2) ? 2 : 1;
          } else {
            def = this.defFor(resolver, p.textureName, p.cell ?? 0, boxSpanHint);
            rotation = p.flipX ? 3 : 0;
            castType = def === INDEX_NONE ? 0 : 1;
          }
        }
        const ax = p.x + p.width / 2;
        // lighting-visual P1: the record's world anchor is the DRAWN art's opaque bottom —
        // the feet — not the frame box's bottom (the letterboxed master's bottom margin put
        // the plan line ~half a tile south of the feet; I1's table). Emit-only prims (no
        // texture) keep the box bottom.
        // z-positioning P2b (F7): the record holds GAME coordinates — where the prim physically
        // STANDS — not where it is drawn. For a standalone prim that is its own art bottom; for a
        // carried piece it is the CARRIER's, and the gap between the two is the piece's elevation.
        // Everything on the floor takes the first branch with elev 0 — the old expression exactly.
        const ownGround = groundRowOf(p);
        const carrier = p.carrierOf !== undefined ? carrierGround.get(p.carrierOf) : undefined;
        const ay = carrier ? carrier.ay : ownGround + (p.elevation ?? 0);
        const elev = carrier ? Math.max(0, carrier.ay - ownGround) : (p.elevation ?? 0);
        // P5: FINE position — quantise to SIXTEENTHS of a unit in one rounding (carry-safe:
        // the integer unit is the high bits of the same number), so a gliding mover's card
        // and light move smoothly instead of stepping whole units.
        const qx = Math.round((ax / U) * 16);
        const qy = Math.round((ay / U) * 16);
        rec.writePrim(idx, {
          unitX: (qx >> 4) & 0xffff,
          unitY: (qy >> 4) & 0xffff,
          fineX: qx & 15,
          fineY: qy & 15,
          // The LIGHT HEIGHT rides unit.z (the DSL authors tiles; records want units). Omitting it
          // left every emitter at Lz = 0, which degenerates the occlusion solve — the giant streak
          // shadows P1b noted.
          //
          // z-positioning F9: `unit.z` means ONE thing — a DRAWN up-screen shift, per the user's
          // `screen.y = unit.y - unit.z`. A light's authored height is a WORLD height
          // (`SquareCache.height` = "world px above the ground plane"), so it converts HERE, once,
          // and every downstream reader compares like with like. Without this the lane held a world
          // height for lights and a drawn shift for elevated parts — quantities differing by
          // sin(tilt), silently compared against each other in the height test.
          // A light's authored height is a WORLD height; a drawn part's elevation is already a
          // drawn shift. Both land in the same lane, so the light converts and the part does not.
          unitZ: L
            ? Math.min(255, Math.round(drawnForWorldHeight((L.height / SQUARE) * UNITS_PER_TILE)))
            : Math.min(255, Math.floor((elev / U) )),
          // P0b: the sixteenths of the SAME elevation — `unit.z + fine.z/16` is the whole value.
          fineZ: L ? 0 : Math.max(0, Math.min(15, Math.round(((elev / U) % 1) * 16))),
          definition: def,
          rotation,
          castType,
          // z-positioning P0b: the `layer` lane is RETIRED — it carried the pawn part slot and
          // nothing ever read it back, so it was write-only data occupying the nibble `fine.z`
          // now needs. World z-order is unaffected: it lives in the presence SORT below, keyed by
          // the same `zIndex` the painter draws with.
          receiveType: p.textureName ? 2 : 0,
          emitType: L ? 1 : 0,
          intensity: L ? intensityLane(L.intensity) : 0,
          reach: L ? Math.max(1, Math.min(REACH_MAX_TILES, Math.round(L.reach / SQUARE))) : 1,
          seed: Math.floor((p.seed ?? 0) * 255) & 0xff,
          // Emitters carry their DSL light RGB in color.1-3 (the slot pass reads it; all-zero
          // falls back to the legacy warmth ramp on color.4, which the debug lights use).
          colors: L
            ? [Math.round(L.color[0] * 255) & 0xff, Math.round(L.color[1] * 255) & 0xff,
               Math.round(L.color[2] * 255) & 0xff, 0]
            : [0, 0, 0, 0],
        });
        const tx = Math.floor(ax / SQUARE);
        const ty = Math.floor(ay / SQUARE);
        if (p.textureName) {
          // P5 (user design): a prim occupies presence in EVERY tile its drawn box
          // x-overlaps — the wolf's 2-tile card must be FOUND by receiver resolution (and
          // the caster walk) from either tile, not just its anchor's. y stays the base row:
          // the card rises north and the consumers already y-scan for the overhang.
          const tx0 = Math.floor(p.x / SQUARE);
          const tx1 = Math.floor((p.x + p.width - 1) / SQUARE);
          for (let t = tx0; t <= tx1; t++) {
            const tkey = (((t + 2048) & 0xffff) << 16) | ((ty + 2048) & 0xffff);
            let list = byTile.get(tkey);
            if (!list) byTile.set(tkey, (list = []));
            list.push({ index: idx, layer: p.zIndex ?? 0 });
          }
        }
        if (L) emitters.push({ index: idx, tileX: tx, tileY: ty,
                               reach: Math.max(1, Math.min(REACH_MAX_TILES, Math.round(L.reach / SQUARE))) });
      }
    }

    // Free records whose cache prim is gone (evicted, despawned, zone closed).
    for (const [key, idx] of this.primByCache) {
      if (!live.has(key)) {
        rec.freePrim(idx);
        this.primByCache.delete(key);
      }
    }

    // Presence: rewrite occupied tiles, clear newly-vacated ones.
    for (const tkey of this.occupied) {
      if (!byTile.has(tkey)) {
        rec.writePresence(((tkey >>> 16) & 0xffff) - 2048, (tkey & 0xffff) - 2048, INDEX_NONE, []);
      }
    }
    for (const [tkey, list] of byTile) {
      rec.writePresence(((tkey >>> 16) & 0xffff) - 2048, (tkey & 0xffff) - 2048, INDEX_NONE, list);
    }
    this.occupied = new Set(byTile.keys());

    // The light map: clear + rebuild whenever the emitter set changes (a mover with a light,
    // a zone eviction, a torch placed). buildLights only writes covered tiles, so without the
    // clear a moved light's OLD coverage lingers as a ghost registration (I1).
    const sig = emitters.map((e) => `${e.index}:${e.tileX}:${e.tileY}:${e.reach}`).join("|");
    if (sig !== this.lightSig) {
      rec.clearLights();
      rec.buildLights(emitters);
      this.lightSig = sig;
    }

    this.defsStale = false;
    rec.upload(gl);
  }

  /** The atlas pages defs live on, first-seen order — bind [0] and [1]; defs past them fall
   *  back conservatively in the shaders. */
  get atlasPages(): Texture[] {
    return this.pages;
  }

  get stats(): Record<string, number> {
    return { defs: this.defByKey.size, prims: this.primByCache.size,
             presenceTiles: this.occupied.size };
  }
}
