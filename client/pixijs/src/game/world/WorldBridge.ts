//! Wiring between the world client and the viewport — the seam that turns
//! subscribed zones into painted tiles, and the viewport's camera into the
//! client's anchor.
//!
//! On every cold-object row delivery (a module's `cold` table — the object model) it
//! runs the DSL to expand the row's `object_kind_reference`s into renderable prims:
//! `onColdObjects` (`Content.zoneColdPrims`, each object's `:visual @on_create`)
//! dispatches by the row's `type_id` — a `biome-tile` row paints a 64×64 ground sprite
//! per cell, a `biome-thing` row a smaller sprite per scattered thing, centred in its
//! cell and above the ground. Rows are tracked per `(zone, type_reference)`, so a zone's
//! several rows (one per biome) coexist and each rewrite re-seeds only itself. On a zone
//! close it drops all of that zone's sprites. The
//! anchor is pushed to the viewport every move (so the camera follows) and to the
//! client whenever it crosses a tile (so the hysteresis ladder subscribes /
//! unsubscribes zones). It also seeds the viewport's static bake config (the noise
//! atlas + the content's material registry), refreshed on a content hot-swap.

import type { Texture } from "pixi.js";
import type { WasmClient, AnchorRadii } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import type { TextureResolver } from "../../textures";
import { SQUARE } from "../viewport/squareMath";
import { MaterialRegistry, type PackedChannel } from "../viewport/material";
import { makeNoiseAtlas } from "../viewport/noiseAtlas";
import { placeThing, readLayout } from "./thingPlacement";

/** The viewport anchor's name in the client's anchor list. */
const ANCHOR = "viewport:0";

/** A well-distributed 32-bit integer hash, normalised to `[0, 1)` — the STABLE per-instance
 *  seed the material bake hashes into a per-instance noise offset. */
function hash01(a: number): number {
  a = Math.imul(a ^ (a >>> 16), 0x45d9f3b);
  a = Math.imul(a ^ (a >>> 16), 0x45d9f3b);
  return ((a ^ (a >>> 16)) >>> 0) / 4294967296;
}

/** A stable seed for a STATIC thing/tile from its world CELL (the same input the worldgen
 *  `tile_seed` uses), so its material variation is fixed to where it sits and never swims. */
function cellSeed(tx: number, ty: number): number {
  return hash01((Math.imul(tx | 0, 73856093) ^ Math.imul(ty | 0, 19349663)) >>> 0);
}

/** How slowly the sticky subscription reach relaxes toward the current zoom's reach —
 *  one tile per this many ms of zoomed-in time. Long enough that a quick zoom in→out
 *  keeps its subs, short enough that a sustained zoom-in lets the outer band fall away. */
const STICKY_RELAX_MS = 200;

/** Whether two radii sets are identical — a zoom that doesn't change the reach skips the
 *  client re-push (and its ladder re-evaluation). */
const sameRadii = (a: AnchorRadii, b: AnchorRadii): boolean =>
  a.active === b.active && a.hot === b.hot && a.warm === b.warm && a.cold === b.cold;

/** Base zIndex for thing sprites — above the ground (tiles are `0`). Each thing
 *  adds its anchor tile-row on top of this, so overlapping things paint in
 *  painter's order (a lower/nearer thing draws over a higher/farther one) and a
 *  big sprite (a tree) that extends into the cell above still sits in front of
 *  whatever grows there. The row comes from {@link placeThing}. */
const THING_Z_BASE = 1;

/** The built-in flat-fill stem: a def that names this (or nothing) has no sprite —
 *  it bakes as a plain tint rect (the geo tier), unchanged from before textures. */
const WHITE_STEM = "white";

/** A prim's texture name for the resolver from its def's stem: the stem itself, or
 *  `undefined` for the built-in white fill / an unnamed def (→ flat tint rect). */
function textureNameFor(stem: string | undefined): string | undefined {
  return stem && stem !== WHITE_STEM ? stem : undefined;
}

/** The facing (a master's `<dir>` segment) + horizontal flip for each 2-bit MOVER
 *  rotation — a pawn's cardinal facing picked from its direction of travel (`0=south`,
 *  matching `FACE_*` on the wire). West ships no master of its own: it reuses the east
 *  master mirrored. */
const FACING_BY_ROTATION: ReadonlyArray<{ facing: "s" | "e" | "n"; flipX: boolean }> = [
  { facing: "s", flipX: false }, // 0 — south
  { facing: "e", flipX: false }, // 1 — east
  { facing: "n", flipX: false }, // 2 — north
  { facing: "e", flipX: true }, //  3 — west (east mirrored)
];

/** The canonical single-facing DIRECTION — the `<dir>` segment a kind that ships ONE
 *  master is authored under. This is the project's DEFAULT facing; it moved from `s`
 *  (south) to `e` (east) when east became the hero facing, so single-facing cold things
 *  (conifer, flora, …) now resolve to `<stem>/e`. Movers still pick a cardinal facing
 *  from their rotation (above) — only single-facing things fold onto this default. */
const DEFAULT_FACING = "e";

/** The category (first stem segment) whose kinds are LINKED/autotiled: their
 *  masters carry the `l` direction and their variation is picked by neighbour context
 *  rather than a facing. Mirrors `bin/art`'s `GRID_CATS`. */
const LINKED_CATEGORY = "linked";

/** A thing's texture name + flip for a given rotation + sprite variant. The def's base
 *  stem gets a trailing DIRECTION segment the resolver treats as its own stem: the
 *  single-facing DEFAULT (`world/conifer` → `world/conifer/e`) for a cold thing, a cardinal
 *  facing from the rotation when `mover` (a pawn's `world/wolf` → `world/wolf/s|e|n`), or `l`
 *  for a linked-category kind (`linked/wall_smooth` → `linked/wall_smooth/l`). `cell` selects
 *  which grid cell of the master atlas to bake: for a facing kind that's the packed
 *  **variant** (which sprite of the kind — the resolver takes it modulo the kind's variant
 *  count); for a linked kind it's the neighbour-context cell (still Phase 2 — canonical cell
 *  0). A white/absent stem stays a flat tint rect. */
export function thingTexture(stem: string | undefined, rotation: number, variant: number, mover = false): { name: string | undefined; flipX: boolean; cell?: number } {
  const base = textureNameFor(stem);
  if (!base) return { name: undefined, flipX: false };
  if (base.startsWith(`${LINKED_CATEGORY}/`)) return { name: `${base}/l`, flipX: false, cell: 0 };
  // A mover (pawn) picks its cardinal facing from the rotation; a single-facing cold thing
  // has one master, authored under the DEFAULT_FACING dir.
  const f = mover ? FACING_BY_ROTATION[rotation & 3] : { facing: DEFAULT_FACING, flipX: false };
  return { name: `${base}/${f.facing}`, flipX: f.flipX, cell: variant };
}

export class WorldBridge {
  /** Cold-object sprite ids per cold ROW, keyed `${macroPosition}:${typeReference}`. A zone has
   *  several rows (a biome-tile row per biome + biome-thing rows), each cleared +
   *  repainted independently on delivery, so they coexist without clobbering each other. */
  private readonly coldPrims = new Map<string, number[]>();
  /** Raw cold rows as delivered, keyed like {@link coldPrims} — kept so a content
   *  hot-swap ({@link setContent}) can re-expand every live row through the new corpus
   *  without re-requesting it. */
  private readonly coldRowsRaw = new Map<
    string,
    | { macroPosition: number; layerReference: number; tiles: Uint16Array }
    | { macroPosition: number; layerReference: number; things: Uint32Array }
  >();
  private readonly unsubs: Array<() => void> = [];

  /** Texture-stem tables from the content bundle, indexed by `defId - 1` (the
   *  `defId` each prim carries). Cached so the per-cell expansion loop doesn't
   *  re-cross the wasm boundary for the same table; refreshed on every content
   *  hot-swap ({@link setContent}). */
  private tileStems: string[] = [];
  private thingStems: string[] = [];
  /** Per-thing spatial layout, flat stride-7 (`[fw, fh, ax, ay, size, sx, sy]` per def),
   *  indexed by `defId - 1` (same as the stem tables) — {@link Content.thingLayout}.
   *  Decoded per prim via {@link readLayout} + {@link placeThing}. Refreshed on hot-swap. */
  private thingLayout: Float64Array = new Float64Array();
  /** Per-def packed-channel material bindings, flat stride-8 (`[mat0, tint0, …, mat3,
   *  tint3]` per def), indexed by `defId - 1` — {@link Content.tilePackedChannels} /
   *  `thingPackedChannels`. Sliced per prim into a {@link PackedChannel}[] the albedo bake
   *  reads. Refreshed on hot-swap alongside the stem tables. */
  private tilePacked: Float64Array = new Float64Array();
  private thingPacked: Float64Array = new Float64Array();

  /** Anchor centre, in world px. */
  private anchorX = 0;
  private anchorY = 0;
  /** Last anchor tile pushed to the client (NaN before the first push). */
  private anchorTileX = NaN;
  private anchorTileY = NaN;
  /** Current viewport zoom (screen px per world px); scales the subscription reach. */
  private zoom = 1;
  /** Sticky subscription reach (tiles): grows instantly on zoom-out, relaxes slowly on
   *  zoom-in, so a zoom in→out cycle keeps its subs instead of dropping + re-fetching. */
  private stickyActive = 0;
  private stickyRelaxedMs = 0;
  /** Last radii pushed to the client, so an unchanged reach skips the re-push. */
  private lastRadii: AnchorRadii = { active: -1, hot: -1, warm: -1, cold: -1 };

  constructor(
    private readonly client: WasmClient,
    private content: Content,
    private readonly viewport: Viewport,
    private readonly white: Texture,
    private readonly resolver: TextureResolver,
  ) {
    this.unsubs.push(
      client.onColdTiles((macroPosition, layerReference, tiles) =>
        this.onColdTiles(macroPosition, layerReference, tiles),
      ),
    );
    this.unsubs.push(
      client.onColdThings((macroPosition, layerReference, things) =>
        this.onColdThings(macroPosition, layerReference, things),
      ),
    );
    this.unsubs.push(client.onZoneClosed((macroPosition) => this.onZoneClosed(macroPosition)));
    // A thing's box is fixed by its def layout (see placeThing), so a texture tier landing
    // doesn't change geometry — the SquareCache re-bakes the geo→master swap itself.
    // The tiling noise atlas the material bake samples — a static, content-independent
    // asset, generated once for the session.
    this.viewport.setNoiseAtlas(makeNoiseAtlas());
    this.refreshStems();
  }

  /** Re-read the per-def texture-stem / packed-channel tables + the material registry from
   *  the current content bundle — after construction and on every hot-swap, so the
   *  expansion loops resolve each prim's `defId` to its stem + material bindings without a
   *  wasm call per cell, and the albedo bake sees any newly-authored materials. */
  private refreshStems(): void {
    this.tileStems = this.content.tileTextureStems();
    this.thingStems = this.content.thingTextureStems();
    this.thingLayout = this.content.thingLayout();
    this.tilePacked = this.content.tilePackedChannels();
    this.thingPacked = this.content.thingPackedChannels();
    this.viewport.setMaterialRegistry(
      MaterialRegistry.fromWasm(
        this.content.materialNoiseFields(),
        this.content.materialSampleSpaces(),
        this.content.materialSwings(),
      ),
    );
  }

  /** Slice a prim's up-to-4 packed-channel bindings out of a stride-8 per-def table by
   *  `defId`, or `undefined` when the def binds no material (all channels empty) — the
   *  common case, so most prims carry no `packed` and bake flat. */
  private packedFor(table: Float64Array, defId: number): PackedChannel[] | undefined {
    const base = (defId - 1) * 8;
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

  /** A thing's zIndex from its anchor tile row — higher (nearer the camera-bottom)
   *  rows paint later, so overlapping things z-order front-over-back. */
  private thingZ(anchorRow: number): number {
    return THING_Z_BASE + anchorRow;
  }

  /** Begin: centre the anchor on tile `(tileX, tileY)` (default the world origin) and
   *  force the first client subscription. Call after login. `?x=/?y=` (debug/urlParams)
   *  jump the camera straight to a cell instead of panning there. */
  start(tileX = 0, tileY = 0): void {
    this.anchorTileX = NaN; // force the first client push
    this.setAnchor(tileX * SQUARE, tileY * SQUARE);
  }

  /** Move the anchor to a world-px point: always recenter the viewport; push to the
   *  client only when the anchor tile OR the reach ({@link radii}) actually changes, so a
   *  pan within a tile and a zoom that doesn't move a tier boundary are both free. */
  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
    this.viewport.setAnchor(x, y);

    const tileX = Math.round(x / SQUARE);
    const tileY = Math.round(y / SQUARE);
    const r = this.radii();
    if (tileX !== this.anchorTileX || tileY !== this.anchorTileY || !sameRadii(r, this.lastRadii)) {
      this.anchorTileX = tileX;
      this.anchorTileY = tileY;
      this.lastRadii = r;
      this.client.setAnchor(ANCHOR, tileX, tileY, r, 0);
    }
  }

  /** Pan the anchor by a world-px delta (drag handling lives in the scene). */
  moveBy(dx: number, dy: number): void {
    this.setAnchor(this.anchorX + dx, this.anchorY + dy);
  }

  /** Apply a zoom: record it (widens {@link radii} as we zoom out to see more world) and
   *  move to the cursor-anchored point. {@link setAnchor} re-pushes on its own iff the
   *  reach actually changed, so a zoom that stays within a tier boundary is free. */
  zoomTo(x: number, y: number, zoom: number): void {
    this.zoom = zoom;
    this.setAnchor(x, y);
  }

  /** Drop everything: listeners, every zone's sprites, and the client anchor. */
  dispose(): void {
    for (const unsub of this.unsubs) unsub();
    this.unsubs.length = 0;
    for (const ids of this.coldPrims.values()) {
      for (const id of ids) this.viewport.removePrim(id);
    }
    this.coldPrims.clear();
    this.coldRowsRaw.clear();
    this.client.removeAnchor(ANCHOR);
  }

  /** Swap in a hot-reloaded content bundle and repaint every live zone through
   *  it — re-expanding the cached raw tiles / things so a `.rd` edit (a recolour, a
   *  new thing tint) shows without re-requesting anything from the server. Each
   *  `onZone*` re-run clears the old sprites and redraws from the new corpus. */
  setContent(content: Content): void {
    this.content = content;
    this.refreshStems(); // the new corpus may retexture / recolour defs
    for (const row of this.coldRowsRaw.values()) {
      if ("tiles" in row) this.onColdTiles(row.macroPosition, row.layerReference, row.tiles);
      else this.onColdThings(row.macroPosition, row.layerReference, row.things);
    }
  }

  // ── internals ───────────────────────────────────────────────────────

  /** Per-tier anchor reach in tiles. `active` (opens subs) + `hot` (pan hysteresis)
   *  track the CURRENT zoom: `active` covers the visible area (half the larger screen
   *  dimension in tiles, ÷ zoom — zooming out shows more world) plus a prefetch margin.
   *  `warm`/`cold` (the sticky candidate band) track a STICKY reach that grows instantly
   *  on zoom-out and relaxes slowly on zoom-in, so a quick zoom in→out keeps its subs
   *  (held as warm candidates — the client's warmth + capacity-LRU bound them) instead
   *  of dropping + re-fetching. Called every anchor move; mutates the sticky state. */
  private radii(): AnchorRadii {
    const screenTiles = Math.max(window.innerWidth, window.innerHeight) / (SQUARE * this.zoom);
    const active = Math.ceil(screenTiles / 2) + 2;
    const now = Date.now();
    if (active >= this.stickyActive) {
      this.stickyActive = active; // zoom-out (or first aim): grow to cover instantly
      this.stickyRelaxedMs = now;
    } else {
      // Zoom-in: relax the held reach toward `active` at one tile per STICKY_RELAX_MS,
      // carrying the remainder so the rate is wall-clock, not call-frequency, driven.
      const shrink = Math.floor((now - this.stickyRelaxedMs) / STICKY_RELAX_MS);
      if (shrink > 0) {
        this.stickyActive = Math.max(active, this.stickyActive - shrink);
        this.stickyRelaxedMs += shrink * STICKY_RELAX_MS;
      }
    }
    return { active, hot: active + 2, warm: this.stickyActive + 4, cold: this.stickyActive + 6 };
  }

  /** A zone's cold **ground** arrived — the dense 256 `kind_reference`s, one per cell. Paints a
   *  64×64 sprite per non-empty cell. Keyed per `(zone, layer)` so a re-delivery clears + repaints
   *  only itself. */
  private onColdTiles(macroPosition: number, layerReference: number, tiles: Uint16Array): void {
    const key = `${macroPosition}:${layerReference}`;
    this.clearColdPrims(key);
    this.coldRowsRaw.set(key, { macroPosition, layerReference, tiles }); // keep for hot-swap re-expand

    // [tileX, tileY, tint, geoColor, defId, …] — stride 5.
    const flat = this.content.zoneTilePrims(macroPosition, tiles);
    const ids: number[] = [];
    for (let i = 0; i + 4 < flat.length; i += 5) {
      const tileX = flat[i];
      const tileY = flat[i + 1];
      const tint = flat[i + 2];
      const geoColor = flat[i + 3];
      const defId = flat[i + 4];
      ids.push(
        this.viewport.addPrim({
          texture: this.white,
          textureName: textureNameFor(this.tileStems[defId - 1]),
          x: tileX * SQUARE,
          y: tileY * SQUARE,
          width: SQUARE,
          height: SQUARE,
          tint,
          geoColor,
          packed: this.packedFor(this.tilePacked, defId),
          seed: cellSeed(tileX, tileY),
        }),
      );
    }
    if (ids.length) this.coldPrims.set(key, ids);
  }

  /** A zone's cold **scatter** arrived — sparse `kind_pos_reference`s. Paints a bottom-centred
   *  sprite per thing, above the ground. Keyed per `(zone, layer)`. */
  private onColdThings(macroPosition: number, layerReference: number, things: Uint32Array): void {
    const key = `${macroPosition}:${layerReference}`;
    this.clearColdPrims(key);
    this.coldRowsRaw.set(key, { macroPosition, layerReference, things });

    // [tileX, tileY, tint, geoColor, kindId, data, variant, …] — stride 7.
    const flat = this.content.zoneColdPrims(macroPosition, layerReference, things);
    const ids: number[] = [];
    for (let i = 0; i + 6 < flat.length; i += 7) {
      const tileX = flat[i];
      const tileY = flat[i + 1];
      const tint = flat[i + 2];
      const geoColor = flat[i + 3];
      const kindId = flat[i + 4];
      const data = flat[i + 5];
      const variant = flat[i + 6];
      const tex = thingTexture(this.thingStems[kindId - 1], data, variant);
      // Anchor/size/footprint from the def layout; a tall sprite rises past its cell,
      // a west facing mirrors the pivot with the art (see placeThing).
      const p = placeThing(tileX, tileY, readLayout(this.thingLayout, kindId), tex.flipX);
      ids.push(
        this.viewport.addPrim({
          texture: this.white,
          textureName: tex.name,
          flipX: tex.flipX,
          cell: tex.cell,
          x: p.x,
          y: p.y,
          width: p.width,
          height: p.height,
          tint,
          geoColor,
          packed: this.packedFor(this.thingPacked, kindId),
          // Cold objects don't move → seed by their world cell (their tile_seed).
          seed: cellSeed(tileX, tileY),
          zIndex: this.thingZ(p.zRow),
        }),
      );
    }
    if (ids.length) this.coldPrims.set(key, ids);
  }

  /** A zone's subscription closed: drop all of its cold-object sprites (every row). */
  private onZoneClosed(macroPosition: number): void {
    const prefix = `${macroPosition}:`;
    for (const key of [...this.coldPrims.keys()]) {
      if (key.startsWith(prefix)) this.clearColdPrims(key);
    }
    for (const key of [...this.coldRowsRaw.keys()]) {
      if (key.startsWith(prefix)) this.coldRowsRaw.delete(key);
    }
  }

  /** Remove one cold ROW's sprites (keyed `${macroPosition}:${typeReference}`) and forget them. */
  private clearColdPrims(key: string): void {
    const ids = this.coldPrims.get(key);
    if (ids) {
      for (const id of ids) this.viewport.removePrim(id);
      this.coldPrims.delete(key);
    }
  }
}
