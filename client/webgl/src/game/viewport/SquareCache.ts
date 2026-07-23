//! The square cache (webgl port) — a FIXED-SIZE, toroidal render-texture cache of the world,
//! baked from primitives on the {@link SQUARE} grid. Channel-agnostic: it bakes N sibling
//! CHANNELS (albedo/normal/surface/zdepth) from ONE shared prim index, window, dirty queue and
//! wrap apron via a merged MRT bake. Ported from the pixijs `SquareCache`; the toroidal window /
//! dirty / apron / fillDisplay logic is identical, only the GPU ops change (Pixi RenderTexture/
//! Mesh/Sprite → engine RenderTarget/Program/draw).
//!
//! SIMPLIFICATION vs pixijs (W4c): one buffer per channel (no ping-pong) and NO reproject — a
//! zoom/LOD re-partition clears + re-bakes the whole window (a brief flash on zoom) instead of
//! reprojecting the old content. Pan is unaffected (leading-edge re-bake only). The ping-pong +
//! reproject smooth-LOD path is a deferred follow-up.
//!
//! COORD CONVENTION: composite + scratch pixel coords have y increasing the same way GL texture
//! rows do; the display's uProjection maps world→screen. Any net Y flip is corrected there.
//!
//! MAP MODEL: these channels are the **textile_square maps** (map-model.md) — `SQUARE` textiles/tile,
//! 1 textile = 1 px — on the shared toroidal `cols × rows` TILE window ({@link window}) that the
//! textile_tile maps (presence/buckets/dirty) and the textile_unit shadow map align to tile-for-tile.
//! Only deviation from pure `cols·SQUARE × rows·SQUARE`: each slot carries a {@link PAD} seam gutter,
//! so the atlas pitch is `SLOT_PW = SQUARE + 2·PAD` (sampling uses the interior only).

import { Renderer, RenderTarget, Program, Geometry, Texture, TexFrame } from "../../gl";
import { MrtBakeShader } from "./mrtBakeShader";
import type { PackedChannel } from "./material";
import {
  mod,
  OVERSCAN,
  sameRange,
  SQUARE,
  squaresForAABB,
  squareWorldX,
  squareWorldY,
  type SquareRange,
} from "./squareMath";
import { ZOOM_MAX } from "../../textures/lod";

/** A renderable occupant of the map. Baked into every square its AABB overlaps. */
export interface Primitive {
  readonly id: number;
  texture: Texture;
  textureName?: string;
  x: number;
  y: number;
  width: number;
  height: number;
  tint: number;
  geoColor?: number;
  flipX?: boolean;
  cell?: number;
  packed?: readonly PackedChannel[];
  seed?: number;
  zIndex: number;
}

/** What a channel's resolve returns — the merged bake gathers albedo material + normal + depth. */
export interface ResolvedPrim {
  texture: Texture;
  tint: number;
  material?: MaterialResolve;
  depth?: number;
  /** The normal map as an atlas sub-frame (or null → flat-up fallback). */
  normal?: { rgb: TexFrame | null };
  surface?: boolean;
}

/** A prim's albedo material — each map is an atlas sub-frame ({@link TexFrame}); the bake reads its
 *  `uvRect()` so a real sprite samples its packed region and the geo tier samples the whole white fill. */
export interface MaterialResolve {
  residual: TexFrame;
  layers: TexFrame | null;
  surface: TexFrame;
  chA: Float32Array;
  chB: Float32Array;
}

export type PrimitiveSpec = Omit<Primitive, "id" | "tint" | "zIndex"> &
  Partial<Pick<Primitive, "tint" | "zIndex">>;

export interface ChannelSpec {
  key: string;
  resolve: (prim: Primitive) => ResolvedPrim;
}

interface PrimEntry {
  prim: Primitive;
  range: SquareRange;
}

const sqKey = (c: number, r: number): string => `${c},${r}`;

const SIZE_STEP = 4;
const RESERVE_CSS = 2 * (OVERSCAN + 1) * SQUARE * ZOOM_MAX;
const PRIO_HIGH = 0;
const PRIO_STD = 256;
const RING_MAX = 255;

// ── the blit program (scratch attachment → composite slot; verbatim copy) ───────────
const BLIT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;             // unit quad 0..1
uniform vec4 uDst;             // dest NDC rect: x0, y0, w, h
uniform vec4 uSrcRect;         // source UV rect: u0, v0, w, h
out vec2 vUV;
void main() {
  vUV = uSrcRect.xy + aPosition * uSrcRect.zw;
  vec2 ndc = uDst.xy + aPosition * uDst.zw;
  gl_Position = vec4(ndc, 0.0, 1.0);
}
`;
const BLIT_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform sampler2D uSrc;
out vec4 fragColor;
void main() { fragColor = texture(uSrc, vUV); }
`;

/** One bake target: a fixed-size buffer + the resolve hook. */
class Channel {
  readonly key: string;
  readonly resolve: (prim: Primitive) => ResolvedPrim;
  buf: RenderTarget | null = null;

  constructor(spec: ChannelSpec) {
    this.key = spec.key;
    this.resolve = spec.resolve;
  }

  ensureBuffer(cw: number, ch: number, gl: WebGL2RenderingContext): void {
    this.buf?.destroy();
    this.buf = new RenderTarget(gl, { width: cw, height: ch, formats: ["rgba8unorm"] });
    this.buf.clear(0, 0, 0, 1); // opaque (see the stale-slot fix note in pixijs)
  }

  destroy(): void {
    this.buf?.destroy();
    this.buf = null;
  }
}

export class SquareCache {
  private readonly channels: Channel[];
  private fixedCW = 0;
  private fixedCH = 0;
  private mrtScratch: RenderTarget | null = null;

  // role refs (found from keys) — the merged bake gathers these; blit order = [albedo,surface,normal,depth]
  private albedoCh: Channel | undefined;
  private surfaceCh: Channel | undefined;
  private normalCh: Channel | undefined;
  private depthCh: Channel | undefined;

  private cols = 0;
  private rows = 0;
  private viewW = 0;
  private viewH = 0;
  private slotPx = SQUARE;

  private winCol = 0;
  private winRow = 0;
  private aimed = false;

  private slotOwnerCol = new Int32Array(0);
  private slotOwnerRow = new Int32Array(0);
  private slotBaked = new Uint8Array(0);

  private nextId = 1;
  private readonly prims = new Map<number, PrimEntry>();
  private readonly squarePrims = new Map<string, Set<number>>();
  private readonly dirty = new Map<string, number>();

  // GPU scratch
  private readonly gl: WebGL2RenderingContext;
  private readonly renderer: Renderer;
  private readonly empty: Texture;
  private readonly blit: Program;
  private readonly unitQuad: Geometry;
  private readonly bake: MrtBakeShader;

  private noiseTex: Texture | null = null;
  private noiseGlobals: readonly [number, number, number] = [1, 1, 128];

  /** Diagnostics: squares baked on the last {@link bakeDirty}. */
  lastBaked = 0;

  constructor(renderer: Renderer, empty: Texture, channels: ChannelSpec[]) {
    this.renderer = renderer;
    this.gl = renderer.gl;
    this.empty = empty;
    this.channels = channels.map((c) => new Channel(c));
    this.albedoCh = this.channels.find((c) => c.key.startsWith("albedo"));
    this.surfaceCh = this.channels.find((c) => c.key.startsWith("surface"));
    this.normalCh = this.channels.find((c) => c.key.startsWith("normal"));
    this.depthCh = this.channels.find((c) => c.key.startsWith("zdepth"));
    this.blit = new Program(this.gl, BLIT_VERT, BLIT_FRAG, "square-blit");
    this.unitQuad = new Geometry(this.gl, this.blit, {
      aPosition: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.bake = new MrtBakeShader(this.gl);
  }

  setNoise(texture: Texture | null, rows: number, uvTile: number, worldTile: number): void {
    this.noiseTex = texture;
    this.noiseGlobals = [rows, uvTile, worldTile];
  }

  displayComposite(key: string): Texture | null {
    const ch = this.channels.find((c) => c.key === key);
    return ch?.buf?.textures[0] ?? null;
  }

  private prio(wc: number, wr: number, band: number): number {
    const cc = this.winCol + (this.cols >> 1);
    const cr = this.winRow + (this.rows >> 1);
    const ring = Math.min(Math.max(Math.abs(wc - cc), Math.abs(wr - cr)), RING_MAX);
    return band + ring;
  }

  private markDirty(wc: number, wr: number, band: number): void {
    if (wc < 0 || wr < 0) return;
    const k = sqKey(wc, wr);
    const p = this.prio(wc, wr, band);
    const cur = this.dirty.get(k);
    if (cur === undefined || p < cur) this.dirty.set(k, p);
  }

  get ready(): boolean {
    return this.cols > 0 && this.channels.length > 0 && this.channels[0].buf != null;
  }

  get displayQuadCount(): number {
    return this.cols * this.rows;
  }

  /** The toroidal tile window geometry — for a sibling world-space toroidal map (shadow-cold) that must
   *  align tile-for-tile with this cache: `cols`/`rows` tiles (incl. `OVERSCAN`), the window origin
   *  (`winCol`/`winRow`, in world tiles), and the atlas slot px. */
  get window(): { winCol: number; winRow: number; cols: number; rows: number; slotPx: number } {
    return { winCol: this.winCol, winRow: this.winRow, cols: this.cols, rows: this.rows, slotPx: this.slotPx };
  }

  // ── sizing ───────────────────────────────────────────────────────────────────
  resize(screenW: number, screenH: number, zoom: number, anchorX: number, anchorY: number): void {
    if (screenW <= 0 || screenH <= 0) return;
    this.ensureBuffers(screenW, screenH);

    const worldW = screenW / zoom;
    const worldH = screenH / zoom;
    this.viewW = worldW;
    this.viewH = worldH;
    const units = (extent: number): number =>
      Math.ceil(Math.ceil(extent / SQUARE) / SIZE_STEP) * SIZE_STEP + 2 * OVERSCAN;
    const cols = units(worldW);
    const rows = units(worldH);
    const slotPx = Math.max(1, Math.min(Math.floor(this.fixedCW / (cols + 2)), Math.floor(this.fixedCH / (rows + 2))));
    if (cols === this.cols && rows === this.rows && slotPx === this.slotPx && this.aimed) return;

    // Re-partition: clear buffers + re-bake the whole window (no reproject — simplified W4c).
    for (const ch of this.channels) ch.buf!.clear(0, 0, 0, 1);
    this.cols = cols;
    this.rows = rows;
    this.slotPx = slotPx;
    this.mrtScratch?.destroy();
    this.mrtScratch = new RenderTarget(this.gl, {
      width: slotPx, height: slotPx,
      formats: ["rgba8unorm", "rgba8unorm", "rgba8unorm", "rgba8unorm"],
    });
    this.slotOwnerCol = new Int32Array(cols * rows);
    this.slotOwnerRow = new Int32Array(cols * rows);
    this.slotBaked = new Uint8Array(cols * rows);
    this.dirty.clear();
    // Aim the window on the anchor + dirty the whole grid (recenter will no-op).
    this.winCol = Math.floor((anchorX - worldW / 2) / SQUARE) - OVERSCAN;
    this.winRow = Math.floor((anchorY - worldH / 2) / SQUARE) - OVERSCAN;
    this.aimed = true;
    this.markStale();
  }

  private ensureBuffers(screenW: number, screenH: number): void {
    const cw = Math.ceil(screenW + RESERVE_CSS);
    const ch = Math.ceil(screenH + RESERVE_CSS);
    if (this.channels[0]?.buf && cw === this.fixedCW && ch === this.fixedCH) return;
    this.fixedCW = cw;
    this.fixedCH = ch;
    for (const channel of this.channels) channel.ensureBuffer(cw, ch, this.gl);
    this.cols = 0;
    this.rows = 0;
    this.slotPx = 0;
    this.aimed = false;
    this.dirty.clear();
  }

  // ── window ────────────────────────────────────────────────────────────────────
  recenter(anchorWorldX: number, anchorWorldY: number): void {
    if (!this.mrtScratch) return;
    const newCol = Math.floor((anchorWorldX - this.viewW / 2) / SQUARE) - OVERSCAN;
    const newRow = Math.floor((anchorWorldY - this.viewH / 2) / SQUARE) - OVERSCAN;
    if (this.aimed && newCol === this.winCol && newRow === this.winRow) return;
    this.winCol = newCol;
    this.winRow = newRow;
    this.aimed = true;
    this.markStale();
  }

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
          this.markDirty(wc, wr, PRIO_HIGH);
        }
      }
    }
  }

  // ── primitive index ─────────────────────────────────────────────────────────────
  addPrim(spec: PrimitiveSpec): number {
    const id = this.nextId++;
    const prim: Primitive = {
      id,
      texture: spec.texture,
      textureName: spec.textureName,
      geoColor: spec.geoColor,
      x: spec.x,
      y: spec.y,
      width: spec.width,
      height: spec.height,
      tint: spec.tint ?? 0xffffff,
      flipX: spec.flipX,
      cell: spec.cell,
      packed: spec.packed,
      seed: spec.seed,
      zIndex: spec.zIndex ?? 0,
    };
    const range = squaresForAABB(prim.x, prim.y, prim.x + prim.width, prim.y + prim.height);
    this.prims.set(id, { prim, range });
    this.linkRange(id, range);
    this.dirtyRange(range);
    return id;
  }

  getPrim(id: number): Primitive | null {
    return this.prims.get(id)?.prim ?? null;
  }

  standingPrims(): Primitive[] {
    const out: Primitive[] = [];
    for (const { prim } of this.prims.values()) if (prim.zIndex >= 1) out.push(prim);
    return out;
  }

  refreshPrim(id: number): void {
    const e = this.prims.get(id);
    if (!e) return;
    const range = squaresForAABB(e.prim.x, e.prim.y, e.prim.x + e.prim.width, e.prim.y + e.prim.height);
    if (!sameRange(range, e.range)) {
      this.unlinkRange(id, e.range);
      this.dirtyRange(e.range);
      e.range = range;
      this.linkRange(id, range);
    }
    this.dirtyRange(e.range);
  }

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
      for (let row = r.row0; row <= r.row1; row++) this.markDirty(c, row, PRIO_STD);
  }

  invalidateAll(): void {
    for (const k of this.squarePrims.keys()) {
      const ci = k.indexOf(",");
      this.markDirty(+k.slice(0, ci), +k.slice(ci + 1), PRIO_STD);
    }
  }

  // ── bake ─────────────────────────────────────────────────────────────────────────
  bakeDirty(budget: number): void {
    this.lastBaked = 0;
    if (!this.mrtScratch || this.dirty.size === 0) return;
    const order = [...this.dirty.entries()].sort((a, b) => a[1] - b[1]);
    let baked = 0;
    for (const [key] of order) {
      if (baked >= budget) break;
      this.dirty.delete(key);
      const ci = key.indexOf(",");
      const wc = +key.slice(0, ci);
      const wr = +key.slice(ci + 1);
      if (!this.inWindow(wc, wr)) continue;
      this.bakeSquare(wc, wr);
      baked++;
    }
    this.lastBaked = baked;
  }

  private inWindow(wc: number, wr: number): boolean {
    return wc >= this.winCol && wc < this.winCol + this.cols && wr >= this.winRow && wr < this.winRow + this.rows;
  }

  private gather(wc: number, wr: number): Primitive[] {
    const ids = this.squarePrims.get(sqKey(wc, wr));
    if (!ids) return [];
    const list: Primitive[] = [];
    for (const id of ids) {
      const e = this.prims.get(id);
      if (e) list.push(e.prim);
    }
    return list;
  }

  /** Bake ONE world square into every channel: render all its prims into the 4-attachment MRT
   *  scratch (one draw per prim, placed by uModel), then blit each attachment to the channel's
   *  slot (interior + wrap-apron border). */
  private bakeSquare(wc: number, wr: number): void {
    const { cols, rows, slotPx } = this;
    const sx = mod(wc, cols);
    const sy = mod(wr, rows);
    const slotX = (sx + 1) * slotPx;
    const slotY = (sy + 1) * slotPx;
    const si = sy * cols + sx;
    this.slotOwnerCol[si] = wc;
    this.slotOwnerRow[si] = wr;
    this.slotBaked[si] = 1;
    const list = this.gather(wc, wr);
    list.sort((a, b) => a.zIndex - b.zIndex || a.id - b.id);

    // Wrap-apron: an interior EDGE slot is also written to the OPPOSITE border.
    const ax = sx === 0 ? (cols + 1) * slotPx : sx === cols - 1 ? 0 : -1;
    const ay = sy === 0 ? (rows + 1) * slotPx : sy === rows - 1 ? 0 : -1;

    // One MRT pass: clear the scratch, then draw each prim placed by uModel.
    this.mrtScratch!.clear(0, 0, 0, 1);
    const wcOrigin = squareWorldX(wc);
    const wrOrigin = squareWorldY(wr);
    for (let i = 0; i < list.length; i++) {
      const prim = list[i];
      const albedoR = this.albedoCh!.resolve(prim);
      const normalR = this.normalCh!.resolve(prim);
      const depthR = this.depthCh!.resolve(prim);
      const mat = albedoR.material!; // B2: albedo always resolves to a material
      this.bake.setTint(albedoR.tint);
      // Each material map is an atlas sub-frame — pass its page texture + UV rect so a real sprite
      // samples only its packed region (geo maps are the whole white fill → identity rect).
      this.bake.setResidual(mat.residual.source, mat.residual.uvRect());
      this.bake.setLayers(mat.layers?.source ?? null, mat.layers?.uvRect());
      this.bake.setSurface(mat.surface.source, mat.surface.uvRect());
      this.bake.setNoise(this.noiseTex);
      this.bake.setChannels(mat.chA, mat.chB);
      const [nrows, uvTile, worldTile] = this.noiseGlobals;
      this.bake.setNoiseGlobals(nrows, uvTile, worldTile);
      this.bake.setWorldRect(prim.x, prim.y, prim.width, prim.height);
      this.bake.setSeed(prim.seed ?? 0);
      this.bake.setNormal(normalR.normal?.rgb?.source ?? null, normalR.normal?.rgb?.uvRect());
      this.bake.setTileDepth(depthR.depth ?? -1);
      const model = this.primModel(prim, wcOrigin, wrOrigin);
      this.renderer.draw({
        program: this.bake.program,
        geometry: this.unitQuad,
        target: this.mrtScratch!,
        blend: "normal",
        textures: this.bake.textures(this.empty),
        uniforms: (p) => {
          p.uMat3("uModel", model);
          this.bake.apply(p);
        },
      });
    }
    // Blit each attachment to its channel's slot (interior + apron).
    const order = [this.albedoCh!, this.surfaceCh!, this.normalCh!, this.depthCh!];
    for (let k = 0; k < 4; k++) {
      const tex = this.mrtScratch!.textures[k];
      const target = order[k].buf!;
      this.blitSlot(tex, slotX, slotY, target);
      if (ax >= 0) this.blitSlot(tex, ax, slotY, target);
      if (ay >= 0) this.blitSlot(tex, slotX, ay, target);
      if (ax >= 0 && ay >= 0) this.blitSlot(tex, ax, ay, target);
    }
  }

  /** unit quad (0..1) → the prim's world rect → this square's scratch NDC (world square fills
   *  the slotPx scratch, so NDC = (worldLocal/SQUARE)*2 - 1). flipX mirrors about the prim x. */
  private primModel(prim: Primitive, wcOrigin: number, wrOrigin: number): Float32Array {
    const k = 2 / SQUARE;
    let a = prim.width * k;
    let tx = prim.x * k - (wcOrigin * k + 1);
    if (prim.flipX) {
      a = -a;
      tx = (prim.x + prim.width) * k - (wcOrigin * k + 1);
    }
    const d = prim.height * k;
    const ty = prim.y * k - (wrOrigin * k + 1);
    // column-major mat3: col0=(a,0,0) col1=(0,d,0) col2=(tx,ty,1)
    return new Float32Array([a, 0, 0, 0, d, 0, tx, ty, 1]);
  }

  /** Copy the whole slotPx scratch into `target` at composite px (dx,dy), verbatim. */
  private blitSlot(srcTex: Texture, dx: number, dy: number, target: RenderTarget): void {
    const w = this.slotPx / this.fixedCW;
    const h = this.slotPx / this.fixedCH;
    const x0 = (dx / this.fixedCW) * 2 - 1;
    const y0 = (dy / this.fixedCH) * 2 - 1;
    this.renderer.draw({
      program: this.blit,
      geometry: this.unitQuad,
      target,
      blend: "none",
      textures: { uSrc: srcTex },
      uniforms: (p) => {
        p.uVec4("uDst", x0, y0, w * 2, h * 2);
        p.uVec4("uSrcRect", 0, 0, 1, 1);
      },
    });
  }

  // ── display ────────────────────────────────────────────────────────────────────
  /** Fill the display mesh: ONE quad per resident world square, sampling each slot's atlas
   *  region (offset +1 past the apron). A slot never baked for the square is a zero-area quad. */
  fillDisplay(panX: number, panY: number, pos: Float32Array, uv: Float32Array): void {
    const { cols, rows, slotPx, winCol, winRow } = this;
    const ownerCol = this.slotOwnerCol;
    const ownerRow = this.slotOwnerRow;
    const baked = this.slotBaked;
    const cw = this.fixedCW;
    const chh = this.fixedCH;
    let b = 0;
    for (let wc = winCol; wc < winCol + cols; wc++) {
      const sx = mod(wc, cols);
      const x0 = wc * SQUARE + panX;
      const x1 = x0 + SQUARE;
      const ix = (sx + 1) * slotPx;
      const u0 = ix / cw;
      const u1 = (ix + slotPx) / cw;
      for (let wr = winRow; wr < winRow + rows; wr++) {
        const sy = mod(wr, rows);
        const si = sy * cols + sx;
        if (wc < 0 || wr < 0 || baked[si] === 0 || ownerCol[si] !== wc || ownerRow[si] !== wr) {
          for (let kk = 0; kk < 8; kk++) { pos[b + kk] = 0; uv[b + kk] = 0; }
          b += 8;
          continue;
        }
        const y0 = wr * SQUARE + panY;
        const y1 = y0 + SQUARE;
        const iy = (sy + 1) * slotPx;
        const v0 = iy / chh;
        const v1 = (iy + slotPx) / chh;
        pos[b] = x0; pos[b + 1] = y0; pos[b + 2] = x1; pos[b + 3] = y0;
        pos[b + 4] = x1; pos[b + 5] = y1; pos[b + 6] = x0; pos[b + 7] = y1;
        uv[b] = u0; uv[b + 1] = v0; uv[b + 2] = u1; uv[b + 3] = v0;
        uv[b + 4] = u1; uv[b + 5] = v1; uv[b + 6] = u0; uv[b + 7] = v1;
        b += 8;
      }
    }
  }

  destroy(): void {
    for (const ch of this.channels) ch.destroy();
    this.mrtScratch?.destroy();
    this.blit.destroy();
    this.unitQuad.destroy();
    this.bake.destroy();
    this.prims.clear();
    this.squarePrims.clear();
    this.dirty.clear();
  }
}
