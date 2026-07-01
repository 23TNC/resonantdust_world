//! The albedo "map": a FIXED-slot, toroidal render-texture cache of the world's
//! albedo (base colour), baked from primitives on the {@link SQUARE} grid.
//!
//! This is the albedo-only first slice of the old multi-channel `RectComposite`
//! (which also baked normal / emissive / depth / lighting). The mechanism it keeps:
//!
//!   • A fixed `cols × rows` grid of square slots in one RenderTexture that NEVER
//!     pans. World square `(wc,wr)` lives at physical slot `(mod(wc,cols), mod(wr,rows))`.
//!   • {@link recenter} advances the window over the world in whole-square steps as
//!     the anchor moves; the strip that just wrapped onto the trailing edge is the
//!     only thing re-baked (the rest of the cache is already correct).
//!   • Primitives know which squares they occupy ({@link Primitive.range}) and squares
//!     know which primitives occupy them (`squarePrims`), so a dirty square has exactly
//!     what it needs to re-bake.
//!   • Each slot carries a {@link PAD} gutter so the display can sample its interior
//!     with bilinear and never read across a slot boundary.
//!
//! The display side ({@link fillDisplay}) emits one quad per resident world square,
//! mapping it to its own slot — the viewport draws that mesh once per frame.

import {
  Container,
  Matrix,
  Rectangle,
  RenderTexture,
  Sprite,
  Texture,
  type Renderer,
} from "pixi.js";
import {
  mod,
  OVERSCAN,
  PAD,
  sameRange,
  SLOT_PH,
  SLOT_PW,
  SQUARE,
  squaresForAABB,
  squareWorldX,
  squareWorldY,
  type SquareRange,
} from "./squareMath";

/** A renderable occupant of the map. Baked into the albedo of every square its
 *  AABB overlaps. `texture` is tinted by `tint` (so a white texture becomes a flat
 *  colour fill); `zIndex` orders overlapping prims within a square (low → first). */
export interface Primitive {
  readonly id: number;
  texture: Texture;
  /** World-px top-left. */
  x: number;
  y: number;
  width: number;
  height: number;
  tint: number;
  zIndex: number;
}

/** Fields a caller supplies to {@link AlbedoMap.addPrim}; `id` is assigned. */
export type PrimitiveSpec = Omit<Primitive, "id" | "tint" | "zIndex"> &
  Partial<Pick<Primitive, "tint" | "zIndex">>;

interface PrimEntry {
  prim: Primitive;
  range: SquareRange;
}

const sqKey = (c: number, r: number): string => `${c},${r}`;

export class AlbedoMap {
  // ── the fixed-slot composite ────────────────────────────────────────────────
  private composite_: RenderTexture | null = null;
  /** One-square scratch the bake renders into before blitting to a slot. */
  private scratchRT: RenderTexture | null = null;
  private scratchTex: Texture | null = null;

  private cols = 0;
  private rows = 0;
  private viewW = 0;
  private viewH = 0;
  /** Composite render-texture resolution (device px per CSS px). A texel is
   *  `1/res` CSS px, so this sizes the half-texel UV inset in {@link fillDisplay}. */
  private res = 1;

  // ── the window over the world (discrete anchor) ──────────────────────────────
  private winCol = 0;
  private winRow = 0;
  private aimed = false;

  // ── per-slot ownership ───────────────────────────────────────────────────────
  // The world square each physical slot was last baked for (the torus is a fixed
  // grid; the *contents* of a slot change as the window re-addresses it). This is
  // the ground truth {@link markStale} re-derives the dirty set from — so a slot
  // can never be silently left showing a stale world square, and the display can
  // refuse to sample a slot that was never baked for the square it's drawing.
  private slotOwnerCol = new Int32Array(0);
  private slotOwnerRow = new Int32Array(0);
  private slotBaked = new Uint8Array(0);

  // ── primitive ⇄ square index ─────────────────────────────────────────────────
  private nextId = 1;
  private readonly prims = new Map<number, PrimEntry>();
  private readonly squarePrims = new Map<string, Set<number>>();
  private readonly dirty = new Set<string>();

  // ── bake scratch objects (reused) ────────────────────────────────────────────
  private readonly empty = new Container();
  private readonly bakeContainer = new Container();
  private readonly blitSprite = new Sprite();
  private readonly bakePool: Sprite[] = [];

  /** Diagnostics: squares baked on the last {@link bakeDirty}. */
  lastBaked = 0;

  constructor() {
    // Verbatim slot copy — no premultiply/blend, so the scratch (incl. its cleared
    // gutter) overwrites the destination slot exactly.
    this.blitSprite.blendMode = "none";
  }

  /** The albedo composite RT — the viewport's display mesh samples it. Never slides. */
  get composite(): RenderTexture | null {
    return this.composite_;
  }

  /** Physical slot counts. */
  get gridCols(): number {
    return this.cols;
  }
  get gridRows(): number {
    return this.rows;
  }

  /** True once sized + the composite is allocated (geometry can be filled). */
  get ready(): boolean {
    return this.cols > 0 && this.composite_ != null;
  }

  /** Number of display quads {@link fillDisplay} emits — ONE per resident world
   *  square. The viewport sizes its vertex/index buffers from this. */
  get displayQuadCount(): number {
    return this.cols * this.rows;
  }

  // ── sizing ───────────────────────────────────────────────────────────────────
  /** (Re)size to a viewport of `viewW × viewH` px. Re-allocates the scratch +
   *  composite when the square count changes, then re-aims the window (whole-window
   *  re-bake) on the next {@link recenter}. No-op if the count is unchanged. */
  resize(viewW: number, viewH: number, renderer: Renderer): void {
    if (viewW <= 0 || viewH <= 0) return;
    const cols = Math.ceil(viewW / SQUARE) + 2 * OVERSCAN;
    const rows = Math.ceil(viewH / SQUARE) + 2 * OVERSCAN;
    this.viewW = viewW;
    this.viewH = viewH;
    if (cols === this.cols && rows === this.rows && this.composite_) return;
    this.cols = cols;
    this.rows = rows;
    const res = renderer.resolution;
    this.res = res;
    // Padded atlas: each slot is SLOT_PW×SLOT_PH, placed at the INTEGER atlas
    // position sx·PW / sy·PH — no fractional blit.
    const cw = cols * SLOT_PW;
    const ch = rows * SLOT_PH;
    this.scratchRT?.destroy(true);
    this.scratchTex?.destroy();
    this.scratchRT = RenderTexture.create({ width: SLOT_PW, height: SLOT_PH, resolution: res });
    this.scratchTex = new Texture({ source: this.scratchRT.source, dynamic: true });
    this.composite_?.destroy(true);
    this.composite_ = RenderTexture.create({ width: cw, height: ch, resolution: res });
    renderer.render({ container: this.empty, target: this.composite_, clear: true, clearColor: [0, 0, 0, 0] });
    // Fresh slots own nothing yet — the composite was just cleared to transparent.
    this.slotOwnerCol = new Int32Array(cols * rows);
    this.slotOwnerRow = new Int32Array(cols * rows);
    this.slotBaked = new Uint8Array(cols * rows);
    // Re-aim + whole-window re-bake on the next recenter.
    this.aimed = false;
    this.dirty.clear();
  }

  // ── window (discrete anchor) ──────────────────────────────────────────────────
  /** Move the window so it stays centred on the anchor's world position. The window
   *  top-left advances in whole-square steps; whenever it moves, {@link markStale}
   *  re-dirties every slot that now addresses a different world square than it was
   *  last baked for (the freshly-exposed strips, plus any a prior budget/skip left
   *  stale). Deriving the re-bake set from slot ownership — not the pan delta —
   *  means panning into empty world (or a fast fling) can never leave a slot
   *  showing a stale square; the torus wrap only re-*addresses* a slot, it doesn't
   *  refresh its contents. */
  recenter(anchorWorldX: number, anchorWorldY: number): void {
    if (!this.scratchRT) return;
    const newCol = Math.floor((anchorWorldX - this.viewW / 2) / SQUARE) - OVERSCAN;
    const newRow = Math.floor((anchorWorldY - this.viewH / 2) / SQUARE) - OVERSCAN;
    if (this.aimed && newCol === this.winCol && newRow === this.winRow) return;
    this.winCol = newCol;
    this.winRow = newRow;
    this.aimed = true;
    this.markStale();
  }

  /** Dirty every window slot whose baked owner is not the world square that now
   *  maps to it. One cheap `cols × rows` scan per window move — the re-bake it
   *  schedules is the same handful of squares a strip pass would (only the
   *  re-addressed slots mismatch), but it is derived from ground truth, so it
   *  self-heals any slot an earlier pan left stale instead of trusting the delta. */
  private markStale(): void {
    const { cols, rows } = this;
    const baseC = mod(this.winCol, cols);
    const baseR = mod(this.winRow, rows);
    for (let sr = 0; sr < rows; sr++) {
      const wr = this.winRow + ((sr - baseR + rows) % rows);
      for (let sc = 0; sc < cols; sc++) {
        const wc = this.winCol + ((sc - baseC + cols) % cols);
        const si = sr * cols + sc;
        if (this.slotBaked[si] === 0 || this.slotOwnerCol[si] !== wc || this.slotOwnerRow[si] !== wr) {
          this.dirty.add(sqKey(wc, wr));
        }
      }
    }
  }

  // ── primitive index ───────────────────────────────────────────────────────────
  /** Register a primitive; returns its id. Links it to every square it overlaps and
   *  dirties those squares. */
  addPrim(spec: PrimitiveSpec): number {
    const id = this.nextId++;
    const prim: Primitive = {
      id,
      texture: spec.texture,
      x: spec.x,
      y: spec.y,
      width: spec.width,
      height: spec.height,
      tint: spec.tint ?? 0xffffff,
      zIndex: spec.zIndex ?? 0,
    };
    const range = squaresForAABB(prim.x, prim.y, prim.x + prim.width, prim.y + prim.height);
    this.prims.set(id, { prim, range });
    this.linkRange(id, range);
    this.dirtyRange(range);
    return id;
  }

  /** The mutable primitive for `id` (move it, retint it, swap its texture), or null.
   *  Call {@link refreshPrim} after mutating so the index + dirty set catch up. */
  getPrim(id: number): Primitive | null {
    return this.prims.get(id)?.prim ?? null;
  }

  /** Re-evaluate `id`'s footprint after a mutation: relink if it moved/resized, and
   *  dirty both the old and new squares (its art/tint may also have changed). */
  refreshPrim(id: number): void {
    const e = this.prims.get(id);
    if (!e) return;
    const range = squaresForAABB(e.prim.x, e.prim.y, e.prim.x + e.prim.width, e.prim.y + e.prim.height);
    if (!sameRange(range, e.range)) {
      this.unlinkRange(id, e.range);
      this.dirtyRange(e.range); // re-bake the vacated squares
      e.range = range;
      this.linkRange(id, range);
    }
    this.dirtyRange(e.range); // always: tint/texture may have changed in place
  }

  /** Drop a primitive and dirty the squares it vacated. */
  removePrim(id: number): void {
    const e = this.prims.get(id);
    if (!e) return;
    this.unlinkRange(id, e.range);
    this.dirtyRange(e.range);
    this.prims.delete(id);
  }

  private linkRange(id: number, r: SquareRange): void {
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) {
        const k = sqKey(c, row);
        let s = this.squarePrims.get(k);
        if (!s) this.squarePrims.set(k, (s = new Set()));
        s.add(id);
      }
  }

  private unlinkRange(id: number, r: SquareRange): void {
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) {
        const k = sqKey(c, row);
        const s = this.squarePrims.get(k);
        if (s) {
          s.delete(id);
          if (s.size === 0) this.squarePrims.delete(k);
        }
      }
  }

  private dirtyRange(r: SquareRange): void {
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) this.dirty.add(sqKey(c, row));
  }

  // ── bake ───────────────────────────────────────────────────────────────────────
  /** Re-bake up to `budget` dirty in-window squares. Squares dirtied while off-window
   *  are dropped here rather than baked — a slot only shows the square it currently
   *  addresses, and {@link markStale} re-dirties it from slot ownership if the window
   *  ever re-addresses that slot to this square, so nothing is lost by dropping it. */
  bakeDirty(renderer: Renderer, budget: number): void {
    this.lastBaked = 0;
    if (!this.scratchRT || this.dirty.size === 0) return;
    let baked = 0;
    const done: string[] = [];
    for (const key of this.dirty) {
      if (baked >= budget) break;
      done.push(key);
      const ci = key.indexOf(",");
      const wc = +key.slice(0, ci);
      const wr = +key.slice(ci + 1);
      if (!this.inWindow(wc, wr)) continue; // owned by a different world square now
      this.bakeSquare(renderer, wc, wr);
      baked++;
    }
    for (const k of done) this.dirty.delete(k);
    this.lastBaked = baked;
  }

  private inWindow(wc: number, wr: number): boolean {
    return wc >= this.winCol && wc < this.winCol + this.cols && wr >= this.winRow && wr < this.winRow + this.rows;
  }

  /** The prims of square `(wc,wr)` AND its 8 neighbours, deduped. The neighbours feed
   *  this slot's gutter (a prim across a boundary must continue into the gutter for
   *  seamless display bilinear); prims that don't reach the padded scratch clip out. */
  private gatherPadded(wc: number, wr: number): Primitive[] {
    const seen = new Set<number>();
    const list: Primitive[] = [];
    for (let dc = -1; dc <= 1; dc++)
      for (let dr = -1; dr <= 1; dr++) {
        const ids = this.squarePrims.get(sqKey(wc + dc, wr + dr));
        if (!ids) continue;
        for (const id of ids) {
          if (seen.has(id)) continue;
          seen.add(id);
          const e = this.prims.get(id);
          if (e) list.push(e.prim);
        }
      }
    return list;
  }

  /** Bake ONE world square: render its prims (z-sorted) into the scratch, translated
   *  to slot-local space, then copy the scratch into the square's fixed slot. An
   *  empty square renders nothing → the cleared scratch blits a transparent slot. */
  private bakeSquare(renderer: Renderer, wc: number, wr: number): void {
    const sx = mod(wc, this.cols);
    const sy = mod(wr, this.rows);
    const slotX = sx * SLOT_PW;
    const slotY = sy * SLOT_PH;
    // This slot now shows this world square — the ground truth {@link markStale}
    // and {@link fillDisplay} check against.
    const si = sy * this.cols + sx;
    this.slotOwnerCol[si] = wc;
    this.slotOwnerRow[si] = wr;
    this.slotBaked[si] = 1;
    const list = this.gatherPadded(wc, wr);
    list.sort((a, b) => a.zIndex - b.zIndex || a.id - b.id);

    this.bakeContainer.removeChildren();
    for (let i = 0; i < list.length; i++) {
      const prim = list[i];
      let sprite = this.bakePool[i];
      if (!sprite) {
        sprite = new Sprite();
        this.bakePool[i] = sprite;
      }
      sprite.texture = prim.texture;
      sprite.tint = prim.tint;
      sprite.position.set(prim.x, prim.y); // world px; the transform maps → slot-local
      sprite.width = prim.width;
      sprite.height = prim.height;
      this.bakeContainer.addChild(sprite);
    }
    // +PAD shifts the square's world origin to the slot interior (the gutter is the
    // PAD margin); −squareWorld moves world space into this square's local space.
    const m = new Matrix().translate(PAD - squareWorldX(wc), PAD - squareWorldY(wr));
    renderer.render({ container: this.bakeContainer, target: this.scratchRT!, clear: true, clearColor: [0, 0, 0, 0], transform: m });
    this.blit(renderer, this.scratchTex!, slotX, slotY);
  }

  /** Copy the full scratch (interior + gutter) into the composite slot at `(dx,dy)`.
   *  Verbatim (blendMode "none"), so it replaces whatever the slot held. */
  private blit(renderer: Renderer, srcTex: Texture, dx: number, dy: number): void {
    const f = srcTex.frame;
    f.x = 0;
    f.y = 0;
    f.width = SLOT_PW;
    f.height = SLOT_PH;
    srcTex.updateUvs();
    this.blitSprite.texture = srcTex;
    this.blitSprite.position.set(dx, dy);
    renderer.render({ container: this.blitSprite, target: this.composite_!, clear: false });
  }

  // ── display ──────────────────────────────────────────────────────────────────
  /**
   * Fill the display mesh: ONE quad per resident world square (`cols × rows`). Each
   * quad maps its world square directly to its OWN padded-atlas slot and samples only
   * that slot's gutter-protected interior — so neither the slot-boundary nor the
   * torus-wrap seam can appear.
   *
   * A slot that was never baked for the square being drawn (empty/out-of-world
   * square not yet cleared, or a bake the budget hasn't reached this frame) is
   * emitted as a ZERO-AREA quad — it draws nothing rather than sampling whatever
   * stale world square that slot last held, so panning into ungenerated space
   * shows empty background, never a repeat of earlier terrain.
   *
   * - `pos` — world quad `(wc·N, wr·N)` + pan (smooth panning lives here).
   * - `uv`  — the slot's INTERIOR atlas region `[ix, ix+N]` (`ix = sx·PW + PAD`).
   *
   * `pos`/`uv` are `displayQuadCount·8`-element Float32Arrays (4 verts × xy).
   */
  fillDisplay(panX: number, panY: number, pos: Float32Array, uv: Float32Array): void {
    const cw = this.cols * SLOT_PW;
    const chh = this.rows * SLOT_PH;
    // Half-texel inset: pull the sampled UV rect in by half a texel on every edge
    // so bilinear filtering can never reach past the slot interior into its gutter
    // (a differing neighbour colour) — the interior→gutter bleed that shows as a
    // stray line at a zone boundary. A texel is 1/res CSS px, so half a texel in
    // normalised UVs is 0.5 / (dimension · res).
    const insetU = 0.5 / (cw * this.res);
    const insetV = 0.5 / (chh * this.res);
    let b = 0;
    for (let wc = this.winCol; wc < this.winCol + this.cols; wc++) {
      const sx = mod(wc, this.cols);
      const x0 = wc * SQUARE + panX;
      const x1 = x0 + SQUARE;
      const ix = sx * SLOT_PW + PAD;
      const u0 = ix / cw + insetU;
      const u1 = (ix + SQUARE) / cw - insetU;
      for (let wr = this.winRow; wr < this.winRow + this.rows; wr++) {
        const sy = mod(wr, this.rows);
        const si = sy * this.cols + sx;
        if (this.slotBaked[si] === 0 || this.slotOwnerCol[si] !== wc || this.slotOwnerRow[si] !== wr) {
          // Slot doesn't currently hold this world square — draw nothing here.
          for (let k = 0; k < 8; k++) { pos[b + k] = 0; uv[b + k] = 0; }
          b += 8;
          continue;
        }
        const y0 = wr * SQUARE + panY;
        const y1 = y0 + SQUARE;
        const iy = sy * SLOT_PH + PAD;
        const v0 = iy / chh + insetV;
        const v1 = (iy + SQUARE) / chh - insetV;
        pos[b] = x0; pos[b + 1] = y0; pos[b + 2] = x1; pos[b + 3] = y0;
        pos[b + 4] = x1; pos[b + 5] = y1; pos[b + 6] = x0; pos[b + 7] = y1;
        uv[b] = u0; uv[b + 1] = v0; uv[b + 2] = u1; uv[b + 3] = v0;
        uv[b + 4] = u1; uv[b + 5] = v1; uv[b + 6] = u0; uv[b + 7] = v1;
        b += 8;
      }
    }
  }

  /** Drop the composite + scratch + index. */
  destroy(): void {
    this.composite_?.destroy(true);
    this.scratchRT?.destroy(true);
    this.scratchTex?.destroy();
    for (const s of this.bakePool) s.destroy();
    this.bakePool.length = 0;
    this.blitSprite.destroy();
    this.bakeContainer.destroy();
    this.empty.destroy();
    this.prims.clear();
    this.squarePrims.clear();
    this.dirty.clear();
  }
}
