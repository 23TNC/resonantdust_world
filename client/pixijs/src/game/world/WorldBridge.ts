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

/** A thing's driving-axis footprint in world px when its def sets no `size`
 *  (content emits `0` = unset). Half a tile, so small flora sits within its cell. */
const DEFAULT_THING_PX = SQUARE / 2;

/** Base zIndex for thing sprites — above the ground (tiles are `0`). Each thing
 *  adds its base tile-row on top of this, so overlapping things paint in
 *  painter's order (a lower/nearer thing draws over a higher/farther one) and a
 *  big sprite (a tree) that extends into the cell above still sits in front of
 *  whatever grows there. See {@link thingZ}. */
const THING_Z_BASE = 1;

/** The built-in flat-fill stem: a def that names this (or nothing) has no sprite —
 *  it bakes as a plain tint rect (the geo tier), unchanged from before textures. */
const WHITE_STEM = "white";

/** A prim's texture name for the resolver from its def's stem: the stem itself, or
 *  `undefined` for the built-in white fill / an unnamed def (→ flat tint rect). */
function textureNameFor(stem: string | undefined): string | undefined {
  return stem && stem !== WHITE_STEM ? stem : undefined;
}

/** The facing (a master's `<dir>` segment) + horizontal flip for each 2-bit thing
 *  rotation. Rotation 0 is SOUTH — the single-facing default every existing master
 *  was renamed to — so untouched worldgen data (rotation all-zero) renders south
 *  unchanged. West ships no master of its own: it reuses the east master mirrored. */
const FACING_BY_ROTATION: ReadonlyArray<{ facing: "s" | "e" | "n"; flipX: boolean }> = [
  { facing: "s", flipX: false }, // 0 — south
  { facing: "e", flipX: false }, // 1 — east
  { facing: "n", flipX: false }, // 2 — north
  { facing: "e", flipX: true }, //  3 — west (east mirrored)
];

/** The category (first stem segment) whose kinds are LINKED/autotiled: their
 *  masters carry the `l` direction and their variation is picked by neighbour context
 *  rather than a facing. Mirrors `bin/art`'s `GRID_CATS`. */
const LINKED_CATEGORY = "linked";

/** A thing's texture name + flip for a given rotation + sprite variant. The def's base
 *  stem gets a trailing DIRECTION segment the resolver treats as its own stem: a facing
 *  from the rotation (`world/conifer` → `world/conifer/e`), or `l` for a linked-category
 *  kind (`linked/wall_smooth` → `linked/wall_smooth/l`). `cell` selects which grid cell of
 *  the master atlas to bake: for a facing kind that's the packed **variant** (which sprite
 *  of the kind — the resolver takes it modulo the kind's variant count); for a linked kind
 *  it's the neighbour-context cell (still Phase 2 — canonical cell 0). A white/absent stem
 *  stays a flat tint rect. */
export function thingTexture(stem: string | undefined, rotation: number, variant: number): { name: string | undefined; flipX: boolean; cell?: number } {
  const base = textureNameFor(stem);
  if (!base) return { name: undefined, flipX: false };
  if (base.startsWith(`${LINKED_CATEGORY}/`)) return { name: `${base}/l`, flipX: false, cell: 0 };
  const f = FACING_BY_ROTATION[rotation & 3];
  return { name: `${base}/${f.facing}`, flipX: f.flipX, cell: variant };
}

export class WorldBridge {
  /** Cold-object sprite ids per cold ROW, keyed `${zoneId}:${typeReference}`. A zone has
   *  several rows (a biome-tile row per biome + biome-thing rows), each cleared +
   *  repainted independently on delivery, so they coexist without clobbering each other. */
  private readonly coldPrims = new Map<string, number[]>();
  /** Raw cold rows as delivered, keyed like {@link coldPrims} — kept so a content
   *  hot-swap ({@link setContent}) or a texture-tier load ({@link reseedThings}) can
   *  re-expand every live row through the new corpus without re-requesting it. */
  private readonly coldRowsRaw = new Map<
    string,
    { zoneId: number; typeReference: number; kinds: Uint32Array }
  >();
  private readonly unsubs: Array<() => void> = [];

  /** Texture-stem tables from the content bundle, indexed by `defId - 1` (the
   *  `defId` each prim carries). Cached so the per-cell expansion loop doesn't
   *  re-cross the wasm boundary for the same table; refreshed on every content
   *  hot-swap ({@link setContent}). */
  private tileStems: string[] = [];
  private thingStems: string[] = [];
  /** Per-thing footprint in tiles, indexed by `defId - 1` (same as the stem
   *  tables). `0` = unset → {@link DEFAULT_THING_PX}. Refreshed on hot-swap. */
  private thingSizes: Float64Array = new Float64Array();
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
      client.onColdObjects((zoneId, typeReference, kinds) =>
        this.onColdObjects(zoneId, typeReference, kinds),
      ),
    );
    this.unsubs.push(client.onZoneClosed((zoneId) => this.onZoneClosed(zoneId)));
    // A thing's footprint depends on its texture's aspect (see thingBox), which
    // isn't known until a real tier loads — re-seed the live things when one lands
    // so a square silhouette becomes its true shape (a 1:2 tree) in place.
    this.unsubs.push(this.resolver.onLoad(() => this.reseedThings()));
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
    this.thingSizes = this.content.thingSizes();
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

  /** A thing's DRIVING (min-axis) footprint in world px from its `defId` — its
   *  content `size` (now in PIXELS, e.g. 64 = one tile wide), or
   *  {@link DEFAULT_THING_PX} when unset. */
  private thingSizePx(defId: number): number {
    const px = this.thingSizes[defId - 1];
    return px && px > 0 ? px : DEFAULT_THING_PX;
  }

  /** A thing's on-screen box in world px: the min axis is its `size`
   *  ({@link thingSizePx}); the other axis is that times the texture's aspect, so
   *  a 1:2 conifer draws 1 wide × 2 tall instead of squished into a square. Aspect
   *  comes from the resolved texture (1 until a real tier loads → a square
   *  silhouette that grows into its shape via {@link reseedThings}).
   *
   *  `textureName` is the DIRECTIONAL leaf actually baked ({@link thingTexture} —
   *  `world/conifer/s`), NOT the base stem: aspect must read the same leaf that
   *  loads (per-facing aspects differ; the base stem is never requested, so it would
   *  always read 1 → square). */
  private thingBox(defId: number, textureName: string | undefined): { w: number; h: number } {
    const size = this.thingSizePx(defId);
    const aspect = this.resolver.aspect(textureName); // h / w
    return aspect >= 1 ? { w: size, h: size * aspect } : { w: size / aspect, h: size };
  }

  /** A thing's zIndex from its base tile row — higher (nearer the camera-bottom)
   *  rows paint later, so overlapping things z-order front-over-back. */
  private thingZ(baseTileY: number): number {
    return THING_Z_BASE + baseTileY;
  }

  /** Re-expand every live zone's cold things from their kept raw data — used when a
   *  texture tier lands (aspect changed) so a thing's footprint updates in place.
   *  Cheap: a handful of stems, each firing this a couple times. */
  private reseedThings(): void {
    for (const row of this.coldRowsRaw.values()) {
      this.onColdObjects(row.zoneId, row.typeReference, row.kinds);
    }
  }

  /** Begin: place the anchor at the world origin and force the first client
   *  subscription. Call after login. */
  start(): void {
    this.anchorTileX = NaN; // force the first client push
    this.setAnchor(0, 0);
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
      this.client.setAnchor(ANCHOR, tileX, tileY, 0, r, 0);
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
      this.onColdObjects(row.zoneId, row.typeReference, row.kinds);
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

  /** A zone's cold-object ROW arrived (the object model). A `biome-tile` row paints the
   *  ground (a 64×64 sprite per cell), a `biome-thing` row paints scattered sprites
   *  (smaller, bottom-centred, above the ground); the row's `type_id` picks which. Keyed
   *  per `(zone, type_reference)` so a zone's many rows (one per biome) coexist and each
   *  re-delivery clears + repaints only itself. Unifies the old tiles/things painters. */
  private onColdObjects(zoneId: number, typeReference: number, kinds: Uint32Array): void {
    const key = `${zoneId}:${typeReference}`;
    this.clearColdPrims(key);
    this.coldRowsRaw.set(key, { zoneId, typeReference, kinds }); // keep for hot-swap re-expand

    const isTile = this.content.objectTypeId(typeReference) === this.content.typeBiomeTile();
    // [tileX, tileY, tint, geoColor, kindId, data, variant, …]
    const flat = this.content.zoneColdPrims(zoneId, typeReference, kinds);
    const ids: number[] = [];
    for (let i = 0; i + 6 < flat.length; i += 7) {
      const tileX = flat[i];
      const tileY = flat[i + 1];
      const tint = flat[i + 2];
      const geoColor = flat[i + 3];
      const kindId = flat[i + 4];
      const data = flat[i + 5];
      const variant = flat[i + 6];
      if (isTile) {
        ids.push(
          this.viewport.addPrim({
            texture: this.white,
            textureName: textureNameFor(this.tileStems[kindId - 1]),
            x: tileX * SQUARE,
            y: tileY * SQUARE,
            width: SQUARE,
            height: SQUARE,
            tint,
            geoColor,
            packed: this.packedFor(this.tilePacked, kindId),
            seed: cellSeed(tileX, tileY),
          }),
        );
      } else {
        const tex = thingTexture(this.thingStems[kindId - 1], data, variant);
        const { w, h } = this.thingBox(kindId, tex.name);
        ids.push(
          this.viewport.addPrim({
            texture: this.white,
            textureName: tex.name,
            flipX: tex.flipX,
            cell: tex.cell,
            // Bottom-CENTRED on the cell: base on the cell's bottom edge, so a
            // taller-than-a-cell sprite (a tree) rises past its cell.
            x: tileX * SQUARE + (SQUARE - w) / 2,
            y: tileY * SQUARE + SQUARE - h,
            width: w,
            height: h,
            tint,
            geoColor,
            packed: this.packedFor(this.thingPacked, kindId),
            // Cold objects don't move → seed by their world cell (their tile_seed).
            seed: cellSeed(tileX, tileY),
            zIndex: this.thingZ(tileY),
          }),
        );
      }
    }
    if (ids.length) this.coldPrims.set(key, ids);
  }

  /** A zone's subscription closed: drop all of its cold-object sprites (every row). */
  private onZoneClosed(zoneId: number): void {
    const prefix = `${zoneId}:`;
    for (const key of [...this.coldPrims.keys()]) {
      if (key.startsWith(prefix)) this.clearColdPrims(key);
    }
    for (const key of [...this.coldRowsRaw.keys()]) {
      if (key.startsWith(prefix)) this.coldRowsRaw.delete(key);
    }
  }

  /** Remove one cold ROW's sprites (keyed `${zoneId}:${typeReference}`) and forget them. */
  private clearColdPrims(key: string): void {
    const ids = this.coldPrims.get(key);
    if (ids) {
      for (const id of ids) this.viewport.removePrim(id);
      this.coldPrims.delete(key);
    }
  }
}
