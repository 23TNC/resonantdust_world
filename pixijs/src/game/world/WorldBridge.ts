//! Wiring between the world client and the viewport — the seam that turns
//! subscribed zones into painted tiles, and the viewport's camera into the
//! client's anchor.
//!
//! On every cold-zone delivery it runs the DSL to expand the zone's packed cells
//! into `[tileX, tileY, tint]` triples and adds one sprite each: `onZoneTiles`
//! (`Content.zoneTilePrims`, each tile's `:visual @on_create`) paints a 64×64
//! ground sprite per non-empty cell; `onZoneThings` (`Content.zoneThingPrims`,
//! each thing's `:visual @on_create`) paints a smaller sprite per scattered thing
//! (the worldgen flora), centred in its cell and above the ground. Tiles and
//! things are tracked per zone separately, so a cold rewrite re-seeds each without
//! disturbing the other or the object-shard loose things. On a zone close it drops
//! all of that zone's sprites. The anchor is pushed to the viewport every move (so
//! the camera follows) and to the client whenever it crosses a tile (so the
//! hysteresis ladder subscribes / unsubscribes zones).

import type { Texture } from "pixi.js";
import type { WasmClient, AnchorRadii, FreeThing } from "../../client/WasmClient";
import { RENDER_DELAY_MS } from "../../client/WasmClient";
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

/** How many move-rows to keep per moving loose thing. Covers the recent past (the
 *  render line + the tween's "from" row) and the server's look-ahead, with margin;
 *  older rows are dropped. */
const FREE_THING_SAMPLE_CAP = 32;

/** Travel time per tile, ms — how long a move-row's tween takes. **Must match the
 *  server's `TILE_TRAVEL_MS`** in `debug_mover.rs`: the projector needs the move
 *  speed to tell "moving" from "resting" without a per-row duration field (see
 *  `docs/sync.md`). A real thing will carry this as a movement stat; the debug
 *  mover hard-codes it on both sides. */
const TILE_TRAVEL_MS = 500;

/** One move-row of a loose thing in the event-log model: `valid_at` (= when the
 *  move *departs*) and the destination tile's fractional global coords
 *  (`freeThingPrim`'s `[x, y]`). It's a behavior, not a sample — the thing is
 *  *heading to* `(fx, fy)` starting at `t`, arriving `t + TILE_TRAVEL_MS`. The
 *  projector tweens from the previous row's tile toward this one over that window
 *  and holds after (see {@link projectMotion}). */
interface FreeSample {
  t: number;
  fx: number;
  fy: number;
}

/** The live state of one rendered loose thing: its sprite, latest payload (for a
 *  content re-expand), the position trail, the current on-screen box, and an
 *  appearance signature (a change — new facing, tint, size — rebuilds the sprite;
 *  a plain move just appends a sample and tweens). */
interface FreeThingState {
  primId: number;
  thing: FreeThing;
  samples: FreeSample[];
  sig: string;
  w: number;
  h: number;
  /** The `valid_at`s of this object's live server rows. A delete removes one; the
   *  sprite is dropped only when this empties — a GC reap of old history (or a
   *  same-ms overwrite) deletes a stale *version*, not the object, and must not
   *  make the thing vanish. */
  versions: Set<number>;
}

/** Insert `s` into `samples` keeping them sorted ascending by `valid_at` (`t`),
 *  replacing any same-`t` entry. Drops the oldest beyond {@link FREE_THING_SAMPLE_CAP}
 *  so the trail stays bounded to the most-recent window. Samples arrive unordered
 *  (subscription burst in storage order; redirect delete/insert interleaving), so
 *  the projection can't rely on append order. */
function insertSample(samples: FreeSample[], s: FreeSample): void {
  const at = samples.findIndex((e) => e.t === s.t);
  if (at >= 0) {
    samples[at] = s;
    return;
  }
  let j = samples.length;
  while (j > 0 && samples[j - 1].t > s.t) j--;
  samples.splice(j, 0, s);
  if (samples.length > FREE_THING_SAMPLE_CAP) samples.splice(0, samples.length - FREE_THING_SAMPLE_CAP);
}

/** Project a moving thing's position at render time `t` from its move-rows
 *  (ascending by `valid_at`). Departure semantics: the *current* row (latest with
 *  `valid_at ≤ t`) is the tile it's committed to; it tweens there from the
 *  *previous* row's tile over `[current.t, current.t + travelMs]`, then holds until
 *  the next row departs. That hold is what a rest/dwell renders as — the gap to the
 *  next departure is left still, so a wait never smears into slow drift. Rows with
 *  `valid_at > t` (the look-ahead) are ignored until `t` reaches them. Returns
 *  `null` only for an empty trail. */
function projectMotion(samples: readonly FreeSample[], t: number, travelMs: number): FreeSample | null {
  const n = samples.length;
  if (n === 0) return null;
  // Current row = last departure at or before t.
  let ci = -1;
  for (let i = 0; i < n; i++) {
    if (samples[i].t <= t) ci = i;
    else break;
  }
  // t precedes every committed departure → hold at the earliest known tile.
  if (ci < 0) return samples[0];
  const cur = samples[ci];
  // No previous tile to come from (freshly spawned) → sit at the current tile.
  if (ci === 0) return cur;
  const moveEnd = cur.t + travelMs;
  if (t >= moveEnd) return cur; // arrived — hold (covers dwell/rest)
  const prev = samples[ci - 1];
  const f = (t - cur.t) / travelMs; // in transit — tween from prev toward cur
  return { t, fx: prev.fx + (cur.fx - prev.fx) * f, fy: prev.fy + (cur.fy - prev.fy) * f };
}

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

/** A thing's texture name + flip for a given rotation. The def's base stem gets a
 *  trailing DIRECTION segment the resolver treats as its own stem: a facing from the
 *  rotation (`world/conifer` → `world/conifer/e`), or `l` for a linked-category kind
 *  (`linked/wall_smooth` → `linked/wall_smooth/l`), whose grid cell (variant) is a
 *  neighbour-context atlas — the context pick is Phase 2; the canonical cell renders
 *  for now. A white/absent stem stays a flat tint rect. */
function thingTexture(stem: string | undefined, rotation: number): { name: string | undefined; flipX: boolean; cell?: number } {
  const base = textureNameFor(stem);
  if (!base) return { name: undefined, flipX: false };
  // A linked (autotile) kind is ONE master atlas sampled by cell. The neighbour-context
  // cell pick is Phase 2 — bake the canonical cell 0 for now; rotation doesn't apply.
  if (base.startsWith(`${LINKED_CATEGORY}/`)) return { name: `${base}/l`, flipX: false, cell: 0 };
  const f = FACING_BY_ROTATION[rotation & 3];
  return { name: `${base}/${f.facing}`, flipX: f.flipX };
}

export class WorldBridge {
  /** Tile-sprite ids per subscribed zone, so a close drops exactly that zone. */
  private readonly zonePrims = new Map<number, number[]>();
  /** Cold thing-sprite ids per zone (worldgen flora), tracked apart from tiles so
   *  a cold rewrite re-seeds each independently. */
  private readonly zoneThings = new Map<number, number[]>();
  /** One sprite per loose thing, keyed by `objectId`. Holds a position trail
   *  ({@link FreeThingState.samples}) the render loop interpolates by `valid_at`,
   *  the latest payload (so a content hot-swap can re-expand it), and
   *  `thing.zoneId` so a zone close drops exactly its things. */
  private readonly freeThings = new Map<number, FreeThingState>();
  /** Raw packed tiles / things per zone as delivered — kept so a content hot-swap
   *  ({@link setContent}) can re-expand every live zone through the new corpus
   *  without re-requesting it from the server. */
  private readonly zoneTilesRaw = new Map<number, Uint16Array>();
  private readonly zoneThingsRaw = new Map<number, Uint32Array>();
  private readonly unsubs: Array<() => void> = [];

  /** Texture-stem tables from the content bundle, indexed by `defId - 1` (the
   *  `defId` each prim carries). Cached so the per-cell expansion loop doesn't
   *  re-cross the wasm boundary for the same table; refreshed on every content
   *  hot-swap ({@link setContent}). */
  private tileStems: string[] = [];
  private thingStems: string[] = [];
  /** Per-thing footprint in tiles, indexed by `defId - 1` (same as the stem
   *  tables). `0` = unset → {@link DEFAULT_THING_TILES}. Refreshed on hot-swap. */
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
   *  zoom-in, so a zoom in→out cycle keeps its outer subs instead of dropping + re-fetching. */
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
    this.unsubs.push(client.onZoneTiles((zoneId, tiles) => this.onZoneTiles(zoneId, tiles)));
    this.unsubs.push(client.onZoneThings((zoneId, things) => this.onZoneThings(zoneId, things)));
    this.unsubs.push(client.onZoneClosed((zoneId) => this.onZoneClosed(zoneId)));
    this.unsubs.push(client.onFreeThing((thing) => this.onFreeThing(thing)));
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

  /** Re-expand every live zone's cold things + loose things from their kept raw
   *  data — used when a texture tier lands (aspect changed) so a thing's footprint
   *  updates in place. Cheap: a handful of stems, each firing this a couple times. */
  private reseedThings(): void {
    for (const [zoneId, things] of this.zoneThingsRaw) this.onZoneThings(zoneId, things);
    for (const { thing } of [...this.freeThings.values()]) this.onFreeThing(thing);
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
    for (const ids of this.zonePrims.values()) {
      for (const id of ids) this.viewport.removePrim(id);
    }
    this.zonePrims.clear();
    for (const ids of this.zoneThings.values()) {
      for (const id of ids) this.viewport.removePrim(id);
    }
    this.zoneThings.clear();
    for (const { primId } of this.freeThings.values()) this.viewport.removePrim(primId);
    this.freeThings.clear();
    this.zoneTilesRaw.clear();
    this.zoneThingsRaw.clear();
    this.client.removeAnchor(ANCHOR);
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

  /** A zone's cold tile baseline arrived: re-seed its ground sprites. Clears only
   *  this zone's tile sprites (a later cold row rewrites them) — its things and any
   *  loose things are untouched, they re-seed on their own deliveries. */
  private onZoneTiles(zoneId: number, tiles: Uint16Array): void {
    this.clearPrims(this.zonePrims, zoneId);
    this.zoneTilesRaw.set(zoneId, tiles); // keep for a content hot-swap re-expand

    const flat = this.content.zoneTilePrims(zoneId, tiles); // [x, y, tint, geoColor, defId, …]
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
    if (ids.length) this.zonePrims.set(zoneId, ids);
  }

  /** A zone's cold thing baseline arrived: re-seed its scattered-thing sprites
   *  (the worldgen flora). Each draws smaller than a tile, centred in its cell and
   *  above the ground. Clears only this zone's thing sprites first; an empty
   *  delivery just clears them. */
  private onZoneThings(zoneId: number, things: Uint32Array): void {
    this.clearPrims(this.zoneThings, zoneId);
    this.zoneThingsRaw.set(zoneId, things); // keep for a content hot-swap re-expand

    const flat = this.content.zoneThingPrims(zoneId, things); // [x, y, tint, geoColor, defId, rotation, …]
    const ids: number[] = [];
    for (let i = 0; i + 5 < flat.length; i += 6) {
      const tileX = flat[i];
      const tileY = flat[i + 1];
      const tint = flat[i + 2];
      const geoColor = flat[i + 3];
      const defId = flat[i + 4];
      const rotation = flat[i + 5];
      const tex = thingTexture(this.thingStems[defId - 1], rotation);
      const { w, h } = this.thingBox(defId, tex.name);
      ids.push(
        this.viewport.addPrim({
          texture: this.white,
          textureName: tex.name,
          flipX: tex.flipX,
          cell: tex.cell,
          // Bottom-CENTRED on the cell: horizontally centred, base on the cell's
          // bottom edge, so a taller-than-a-cell sprite (a tree) rises past the
          // cell it occupies rather than overflowing symmetrically.
          x: tileX * SQUARE + (SQUARE - w) / 2,
          y: tileY * SQUARE + SQUARE - h,
          width: w,
          height: h,
          tint,
          geoColor,
          packed: this.packedFor(this.thingPacked, defId),
          // Cold worldgen things don't move → seed by their world cell (their tile_seed).
          seed: cellSeed(tileX, tileY),
          zIndex: this.thingZ(tileY),
        }),
      );
    }
    if (ids.length) this.zoneThings.set(zoneId, ids);
  }

  /** A zone's subscription closed: drop all of its sprites — tiles, cold things,
   *  and any object-shard loose things in that zone. */
  private onZoneClosed(zoneId: number): void {
    this.clearPrims(this.zonePrims, zoneId);
    this.clearPrims(this.zoneThings, zoneId);
    this.zoneTilesRaw.delete(zoneId);
    this.zoneThingsRaw.delete(zoneId);
    for (const [objectId, ft] of this.freeThings) {
      if (ft.thing.zoneId === zoneId) {
        this.viewport.removePrim(ft.primId);
        this.freeThings.delete(objectId);
      }
    }
  }

  /** Remove a zone's sprites from one of the per-zone sprite maps (tiles or cold
   *  things) and forget them. */
  private clearPrims(map: Map<number, number[]>, zoneId: number): void {
    const ids = map.get(zoneId);
    if (ids) {
      for (const id of ids) this.viewport.removePrim(id);
      map.delete(zoneId);
    }
  }

  /** A loose thing changed. On remove, drop the sprite. Otherwise this is an
   *  insert or a move: record the new timestamped position into the object's trail
   *  (the render loop tweens the sprite to `syncedNow − RENDER_DELAY_MS`, so it
   *  glides instead of snapping and every client shows it at the same world point
   *  at the same wall time). The sprite itself is created once and only rebuilt
   *  when its *appearance* changes (a new facing, tint, or size) — a plain move
   *  keeps it and just extends the trail. Keyed by `objectId`. */
  private onFreeThing(thing: FreeThing): void {
    const existing = this.freeThings.get(thing.objectId);
    if (thing.removed) {
      if (!existing) return;
      // Drop this specific version; the object survives as long as any remain (a
      // GC reap of old history, or a same-ms overwrite, deletes a stale version —
      // not the thing). Only an all-versions-gone delete removes the sprite.
      existing.versions.delete(thing.validAt);
      const si = existing.samples.findIndex((s) => s.t === thing.validAt);
      if (si >= 0) existing.samples.splice(si, 1);
      if (existing.versions.size === 0) {
        this.viewport.removePrim(existing.primId);
        this.freeThings.delete(thing.objectId);
      }
      return;
    }

    // [x, y, tint, geoColor, defId] — fractional global tile coords + colour + stem index.
    const prim = this.content.freeThingPrim(thing.zoneId, thing.location, thing.offset, thing.id);
    const defId = prim[4];
    const tex = thingTexture(this.thingStems[defId - 1], thing.rotation);
    const { w, h } = this.thingBox(defId, tex.name);
    const sample: FreeSample = { t: thing.validAt, fx: prim[0], fy: prim[1] };
    // Appearance identity: anything that changes how (not where) it draws. A
    // difference forces a sprite rebuild; a match lets the existing sprite tween.
    const sig = `${defId}|${tex.name ?? ""}|${tex.flipX}|${tex.cell ?? -1}|${w.toFixed(2)}|${h.toFixed(2)}|${prim[2]}|${prim[3]}`;

    if (!existing) {
      const primId = this.spawnFreePrim(thing, prim, tex, w, h);
      this.freeThings.set(thing.objectId, {
        primId,
        thing,
        samples: [sample],
        sig,
        w,
        h,
        versions: new Set([thing.validAt]),
      });
      return;
    }

    existing.thing = thing;
    existing.w = w;
    existing.h = h;
    existing.versions.add(thing.validAt);
    // Insert keeping the trail sorted by `valid_at`. Rows do NOT arrive in order —
    // the subscription delivers the history burst in storage order, and a redirect
    // interleaves deletes + inserts — so a naive append corrupts the projection
    // (which assumes sorted samples). Bounded to the most-recent CAP by time.
    insertSample(existing.samples, sample);
    if (sig !== existing.sig) {
      // Appearance changed: rebuild the sprite. Position is corrected on the next
      // frame's tween, so a one-frame placement at the sample point is harmless.
      this.viewport.removePrim(existing.primId);
      existing.primId = this.spawnFreePrim(thing, prim, tex, w, h);
      existing.sig = sig;
    }
  }

  /** Create the sprite for a loose thing at its sample position (bottom-centred on
   *  its sub-tile cell, like cold things). Position is authoritative only for this
   *  first frame — {@link tick} tweens it thereafter. Returns the prim id. */
  private spawnFreePrim(
    thing: FreeThing,
    prim: ArrayLike<number>,
    tex: { name: string | undefined; flipX: boolean; cell?: number },
    w: number,
    h: number,
  ): number {
    return this.viewport.addPrim({
      texture: this.white,
      textureName: tex.name,
      flipX: tex.flipX,
      cell: tex.cell,
      x: prim[0] * SQUARE + (SQUARE - w) / 2,
      y: prim[1] * SQUARE + SQUARE - h,
      width: w,
      height: h,
      tint: prim[2],
      geoColor: prim[3],
      packed: this.packedFor(this.thingPacked, prim[4]),
      // A MOVER: seed by its stable objectId (NOT position) so its pattern doesn't swim.
      seed: hash01(thing.objectId),
      zIndex: this.thingZ(Math.floor(prim[1])),
    });
  }

  /** Per-frame: tween every loose thing to the shared render instant
   *  `syncedNow − RENDER_DELAY_MS`, interpolating its trail by `valid_at`. Driven
   *  from {@link WorldScene.update}. Cheap and a no-op when nothing's loose. */
  tick(): void {
    if (this.freeThings.size === 0) return;
    const renderAt = this.client.syncedNowMs() - RENDER_DELAY_MS;
    for (const st of this.freeThings.values()) {
      const p = projectMotion(st.samples, renderAt, TILE_TRAVEL_MS);
      if (!p) continue;
      this.viewport.movePrim(
        st.primId,
        p.fx * SQUARE + (SQUARE - st.w) / 2,
        p.fy * SQUARE + SQUARE - st.h,
      );
    }
  }

  /** Swap in a hot-reloaded content bundle and repaint every live zone through
   *  it — re-expanding the cached raw tiles / things / loose things so a `.rd`
   *  edit (a recolour, a new thing tint) shows without re-requesting anything from
   *  the server. Each `onZone*` / `onFreeThing` re-run clears the old sprites and
   *  redraws from the new corpus. */
  setContent(content: Content): void {
    this.content = content;
    this.refreshStems(); // the new corpus may retexture / recolour defs
    for (const [zoneId, tiles] of this.zoneTilesRaw) this.onZoneTiles(zoneId, tiles);
    for (const [zoneId, things] of this.zoneThingsRaw) this.onZoneThings(zoneId, things);
    // Re-expand loose things from their kept payloads (a snapshot first, since
    // onFreeThing mutates the map as it replaces each sprite).
    for (const { thing } of [...this.freeThings.values()]) this.onFreeThing(thing);
  }
}
