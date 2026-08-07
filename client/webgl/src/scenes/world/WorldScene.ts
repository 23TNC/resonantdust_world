//! WorldScene — the real world scene. Hosts the {@link ViewportPanel} (its own canvas + engine
//! Renderer) and wires the {@link WorldBridge} (subscribed-zone cold tiles/things → the cold
//! SquareCache, camera anchor → client subscription) + the {@link MoverLayer} (pawns → the warm
//! cache) + the {@link ChatPanel} (slash-command console). Drag pans, scroll zooms (about the
//! cursor), both routed THROUGH the bridge so zone subscriptions follow the view. URL params
//! (`?focus=x,y`, `?grid`, `?zoom`) replay as chat commands once, after login. The RT-preview +
//! shadow/spike commands need the render-texture debug infra still in W4f.

import { Scene } from "../Scene";
import type { GameContext } from "../../GameContext";
import { ViewportPanel } from "../../game/viewport/ViewportPanel";
import { WorldBridge } from "../../game/world/WorldBridge";
import { MoverLayer } from "../../game/world/MoverLayer";
import { ChatPanel } from "../../game/panels/chat/ChatPanel";
import { DetailsPanel } from "../../game/panels/details/DetailsPanel";
import { BuildPanel } from "../../game/panels/build/BuildPanel";
import { buildMenuEntries, blueprintStemFor, type BuildEntry } from "../../game/world/buildMenu";
import { linkedCell, N as MASK_N, E as MASK_E, S as MASK_S, W as MASK_W } from "../../game/world/linkedCell";
import type { BlueprintTile } from "../../game/viewport/blueprintOverlay";
import { LogManager } from "../../game/panels/chat/LogManager";
import { PanelManager } from "../../ui/panels/PanelManager";
import { SQUARE } from "../../game/viewport/squareMath";
import { SelectionModel, type Selection } from "../../game/world/SelectionModel";
import { PieMenu, type PieMenuOption } from "../../game/panels/PieMenu";
import { tileToPosition, composeInteraction } from "../../client/WasmClient";
import type { OutlineItem } from "../../game/viewport/outlineOverlay";
import { onContentReloaded, getContent } from "../../game/definitions/contentBoot";
import { definitionRegistry } from "../../game/definitions/definitionRegistry";
import { parseUrl, type UrlCommand } from "../../debug/urlParams";

/** Wheel deltaY per zoom octave: factor = 2^(-deltaY/this), applied per event (continuous). */
const WHEEL_OCTAVE = 500;
/** ui-select P0: a left press that travels further than this before release is a drag (today:
 *  discarded; the drag-box pass turns it into box-select), not a click. */
const CLICK_SLOP_PX = 4;
/** Commands whose infra (RT preview, shadow cast, ES 3.00 spikes) lands in W4f — registered so
 *  they're discoverable + don't error, reporting their pending status. */
const PENDING_W4F: Record<string, string> = {
  showRT: "The render-texture preview (/showRT) isn't ported yet — lands with the RT debug panel (W4f).",
  shadowcast: "The shadow-cast experiment (/shadowcast) isn't ported yet (W4f).",
  es300: "The ES 3.00 spike (/es300) isn't ported yet (W4f).",
  mrttest: "The MRT spike (/mrttest) isn't ported yet (W4f).",
  inttest: "The integer-RT spike (/inttest) isn't ported yet (W4f).",
};

export class WorldScene extends Scene {
  private ctx!: GameContext;
  private panel!: ViewportPanel;
  private bridge!: WorldBridge;
  private moverLayer!: MoverLayer;
  private chat!: ChatPanel;
  private details: DetailsPanel | null = null;
  private contentUnsub: (() => void) | null = null;
  private urlCommands: UrlCommand[] = [];
  private dragId: number | null = null;
  private lastClientX = 0;
  private lastClientY = 0;
  /** ui-select P0 (D1): THE selection — outline/details/title-bar/move-order all read this. */
  readonly selection = new SelectionModel();
  /** The one context pie menu (input-rework F1/F7 — at most one, self-dismissing). */
  private readonly pieMenu = new PieMenu();
  /** A pending left CLICK (down seen, up not yet) — up within {@link CLICK_SLOP_PX} selects.
   *  The future drag-box replaces "discard on move" with "start the box" right here. */
  private clickId: number | null = null;
  private clickX = 0;
  private clickY = 0;
  /** build-walls P2 (D4): the active placement mode's kind, or null. While set, left-drag
   *  forms the wall rect (selection clicks suspended) and right-click exits the mode. */
  private buildMode: BuildEntry | null = null;
  /** The drag's start tile once the left button is down in build mode. */
  private buildDragStart: { x: number; y: number } | null = null;
  private buildPanel: BuildPanel | null = null;

  onEnter(ctx: GameContext): void {
    this.ctx = ctx;
    this.panel = new ViewportPanel(ctx);
    this.panel.open();
    // Attach the texture resolver + the viewport's GL context (F6: the viewport self-canvases, so the
    // atlas-backed resolver shares its renderer). Things now resolve master→preview→geo textures.
    this.panel.view.setResolver(ctx.textureResolver);

    // Open-panel registry + the client-only log feed (the chat panel's "logs" tab reads it).
    ctx.panels = new PanelManager();
    ctx.logs = new LogManager();

    // The client zone stream → viewport tiles, and the camera anchor → client subscription.
    this.bridge = new WorldBridge(ctx.client, ctx.content, this.panel.view, this.panel.view.white, ctx.textureResolver);
    // DEBUG: `__zoom(z)` is the ONLY correct way to zoom from a console. Zooming is TWO calls that must
    // both happen — `view().setZoom` moves the camera and returns the new anchor; `bridge.zoomTo` records
    // the zoom so `radii()` sizes the zone subscription for the right partition level. Driving either one
    // alone looks like a client bug: camera-only leaves the subscription sized for the previous level
    // (empty window edges), bridge-only leaves the camera — and hence the level — unchanged. Both cost real time
    // (work `2026-07-26-textile-slot` I11). This mirrors the `/zoom` command exactly.
    (globalThis as unknown as { __zoom: (z: number) => number }).__zoom = (z: number) => {
      const anchor = this.panel.view.setZoom(z);
      if (anchor) this.bridge.zoomTo(anchor.x, anchor.y, this.panel.view.zoom);
      return this.panel.view.zoom;
    };
    // Pawns (the wolves): synced from the tick pipeline's mobile entities into the viewport's WARM cache.
    this.moverLayer = new MoverLayer(ctx.client, ctx.content, this.panel.view);
    // DEBUG: `__sel` — the live SelectionModel (ui-select P0 acceptance probes read/drive it).
    (globalThis as unknown as { __sel: SelectionModel }).__sel = this.selection;
    // DEBUG: `__bridge` — the WorldBridge (build-walls drills drive the tile paths directly).
    (globalThis as unknown as { __bridge: WorldBridge }).__bridge = this.bridge;
    // ui-select P1: a selected TILE shows its world coordinates in the panel's title bar
    // (the "selection" suffix — also visible on the taskbar entry while the bar is hidden).
    this.panel.setTitleSuffixResolver("selection", () => {
      const p = this.selection.primary;
      return p?.kind === "tile" ? `tile ${p.x}, ${p.y}` : null;
    });
    this.selection.subscribe(() => this.panel.refreshTitleSuffix());
    // ui-select P2: the details panel — reads the SelectionModel + live world providers.
    this.details = new DetailsPanel(ctx, this.selection, {
      pawn: (e) => {
        const info = this.moverLayer.pawnInfo(e);
        if (!info) return null;
        // needs-moodlets P5: evaluate the pawn's RAW payload through the ONE wasm eval
        // (F3) at the LEARNED now-tic — lazily, at the panel's own refresh cadence (F4:
        // nothing ticks; the row + the corpus + the tic are the whole computation).
        let conditions: { label: string; mood: number; remaining: number; priority: number }[] = [];
        let mood = 0.5;
        // TWO eval inputs now (stat-model F2): the payload (TRAIT + CONDITION rows) and
        // the fanned `needs` sub-table rows, both verbatim — the host decodes neither.
        const payload = this.moverLayer.pawnPayload(e) ?? new Uint32Array(0);
        const needs = this.moverLayer.pawnNeeds(e);
        const d = ctx.client.ticDelta(0); // fractional now-tic (mod 2^16 below)
        if (d !== null) {
          const now = ((Math.floor(d) % 0x10000) + 0x10000) % 0x10000;
          const c = getContent();
          // Stride 4: [condition_id, mood, remaining, priority]. The eval returns them
          // ALREADY SORTED (conditions F3 — priority desc, |mood| desc, id asc); this loop
          // preserves that order and must never re-sort. `priority` is carried for display.
          const flat = c.pawnConditions(payload, needs, now);
          for (let i = 0; i + 3 < flat.length; i += 4) {
            // `condition_id` is a u32 gameplay definition_reference now (interactions F1) —
            // labels resolve BY REF, never by position.
            const id = flat[i];
            conditions.push({
              label: c.conditionLabelOf(id) ?? `#${id.toString(16)}`,
              mood: flat[i + 1],
              remaining: flat[i + 2],
              priority: flat[i + 3],
            });
          }
          mood = c.pawnMood(payload, needs, now);
        }
        return { ...info, conditions, mood };
      },
      thing: (id) => {
        const t = this.panel.view.coldGetPrim(id);
        return t ? { textureName: t.textureName, x: t.x, y: t.y, width: t.width, height: t.height, zIndex: t.zIndex } : null;
      },
    });
    this.details.open();
    // build-walls P2: the build panel — categories/icons entirely from content (D3).
    this.buildPanel = new BuildPanel(ctx, ctx.textureResolver,
      () => buildMenuEntries(getContent()),
      (entry) => this.enterBuildMode(entry));
    this.buildPanel.open();

    // The corpus hot-swaps on login (`onLoggedIn` → `reloadContent` in main.ts pulls the server's
    // corpus, replacing the boot embed). Push the new corpus into the bridge + mover layer so they
    // RE-EXPAND every delivered cold row against it. Without this, a bridge created before
    // `reloadContent` finished stays stuck on the embed — cold rows for the server's biomes expand
    // to nothing (the intermittent blank world). `getContent()` is the freshly-swapped bundle.
    this.contentUnsub = onContentReloaded(() => {
      const c = getContent();
      this.bridge.setContent(c);
      this.moverLayer.setContent(c);
    });

    // Seed the INITIAL anchor from `?focus=x,y` BEFORE the first subscription, so it opens at the
    // target tile (no origin flash). The replayed `/focus` below then re-applies it as a no-op.
    this.urlCommands = parseUrl().commands;
    const focus = this.urlCommands.find((c) => c.name === "focus");
    const startX = focus ? Number(focus.args[0]) || 0 : 0;
    const startY = focus ? Number(focus.args[1]) || 0 : 0;
    this.bridge.start(startX, startY);

    // The chat console + slash commands, then replay the URL params through them (once).
    this.chat = new ChatPanel(ctx);
    this.chat.open();
    this.registerCommands();
    this.runUrlCommands();

    const canvas = this.panel.canvas;
    canvas.addEventListener("pointerdown", this.onPointerDown);
    canvas.addEventListener("pointermove", this.onPointerMove);
    canvas.addEventListener("pointerup", this.onPointerUp);
    canvas.addEventListener("pointercancel", this.onPointerUp);
    canvas.addEventListener("wheel", this.onWheel, { passive: false });
    // ui-select P0: the RIGHT button is the move order — the browser menu never belongs on
    // the world canvas.
    canvas.addEventListener("contextmenu", this.onContextMenu);
  }

  update(): void {
    // Movers FIRST (hot-sync F2): the chase advances, mutates the warm prims, and raises
    // the hot dirty — THEN the viewport bakes + lights + blits the SAME snapshot. The old
    // order rendered yesterday's mover state and let each hot consumer pick the change up
    // on its own schedule (a full frame of baseline lag, plus divergent cadences).
    this.moverLayer?.tick();
    // ui-select P1: outlines rebuild per frame BETWEEN movers and the draw — a selected
    // pawn's outline follows the prim the chase just moved, same-frame.
    this.syncOutlines();
    this.panel?.tick();
    // Push the live viewport zoom to the debug HUD (textures tab). The scene-
    // independent `setStats` ticker in main.ts has no viewport handle, so drive it
    // from here — covers wheel, `/zoom`, and the initial value in one place.
    this.ctx?.debugPanel?.setZoom(this.panel.view.zoom);
  }

  onExit(): void {
    const canvas = this.panel?.canvas;
    if (canvas) {
      canvas.removeEventListener("pointerdown", this.onPointerDown);
      canvas.removeEventListener("pointermove", this.onPointerMove);
      canvas.removeEventListener("pointerup", this.onPointerUp);
      canvas.removeEventListener("pointercancel", this.onPointerUp);
      canvas.removeEventListener("wheel", this.onWheel);
      canvas.removeEventListener("contextmenu", this.onContextMenu);
    }
    this.contentUnsub?.();
    this.contentUnsub = null;
    this.pieMenu.dispose();
    this.chat?.destroy();
    this.details?.destroy();
    this.details = null;
    this.buildPanel?.destroy();
    this.buildPanel = null;
    this.moverLayer?.dispose();
    this.bridge?.dispose();
    this.panel?.destroy();
  }

  // ── chat commands ─────────────────────────────────────────────────────────────────
  /** Register every slash command against the chat panel. Each is also reachable from the URL
   *  query (`?grid=1` ≡ `/grid 1`). Working commands drive the camera/grid live; the RT/shadow/
   *  spike ones report their W4f-pending status so they're discoverable without erroring. */
  private registerCommands(): void {
    const view = (): ViewportPanel["view"] => this.panel.view;

    // `/grid [0|1|2|3]` — 1 region+zone+tile, 2 region+zone, 3 region, 0 off. No arg toggles.
    const gridLabel = ["Debug grid off.", "Debug grid: region + zone + tile.", "Debug grid: region + zone.", "Debug grid: region."];
    this.chat.registerCommand("grid", (args) => {
      let level: number;
      if (args.length === 0) {
        level = view().debugGrid > 0 ? 0 : 1;
      } else {
        const n = Number(args[0]);
        if (!Number.isInteger(n) || n < 0 || n > 3) {
          return "Usage: /grid [0|1|2|3]  (0 off, 1 region+zone+tile, 2 region+zone, 3 region)";
        }
        level = n;
      }
      view().setDebugGrid(level);
      return gridLabel[level];
    });

    // `/focus <tileX> <tileY>` — jump the camera centre to a tile (through the bridge, so the
    // subscription follows).
    this.chat.registerCommand("focus", (args) => {
      const x = Number(args[0]);
      const y = Number(args[1]);
      if (!Number.isFinite(x) || !Number.isFinite(y)) return "Usage: /focus <tileX> <tileY>";
      this.bridge.setAnchor(x * SQUARE, y * SQUARE);
      return `Camera focused on tile (${x}, ${y}).`;
    });

    // `/zoom <level>` — absolute zoom (1 = native 64px, 2 = 2× in, 0.5 = out), holding the centre.
    this.chat.registerCommand("zoom", (args) => {
      const z = Number(args[0]);
      if (!Number.isFinite(z) || z <= 0) return "Usage: /zoom <level>  (1 = native, 2 = 2× in, 0.5 = out)";
      const anchor = view().setZoom(z);
      if (anchor) this.bridge.zoomTo(anchor.x, anchor.y, view().zoom);
      return `Zoom set to ${view().zoom}×.`;
    });

    // `/overlayRT <channel>` — draw ONE G-buffer composite over the lit world (world-aligned,
    // full-viewport). No arg (or an unknown channel) echoes the valid channels; a valid one
    // switches to it, or turns the overlay off if it's already showing.
    this.chat.registerCommand("overlayRT", (args) => {
      const names = view().overlayChannelNames();
      const name = args[0];
      if (!name) {
        const cur = view().overlayChannel;
        return `Usage: /overlayRT <channel>. ${cur ? `Showing ${cur}. ` : ""}Channels: ${names.join(", ")}.`;
      }
      if (!names.includes(name)) {
        return `Unknown channel "${name}". Channels: ${names.join(", ")}.`;
      }
      const now = view().setOverlay(name);
      return now ? `Overlaying ${now}.` : `Overlay off (${name}).`;
    });

    // `/spawn <thing> [x y] [body N] [head N]` — human-pawns-redux P2 (F2): the dev MINT
    // door, re-opened. Composes `PROMOTE CREATE def pos count [PART(0, body_def)
    // PART(1, head_def)]` through the same allowlisted queue everything rides; a pawn's
    // body/head choice is the PART refs' VARIANT nibble (F3 — one def, u4 appearance).
    // x/y default to the camera centre; variants default 0; single-part kinds mint bare.
    this.chat.registerCommand("spawn", (args) => {
      const usage = "Usage: /spawn <thing> [x y] [body 0-15] [head 0-15]";
      const name = args[0];
      if (!name) return usage;
      const c = getContent();
      const names = c.thingNames();
      const objectId = names.indexOf(name) + 1;
      if (objectId === 0) return `Unknown thing "${name}". Things: ${names.join(", ")}.`;
      const tax = c.thingTaxonomy(objectId);
      if (!tax || tax.length < 4) return `"${name}" has no taxonomy — cannot resolve a def.`;
      const def = definitionRegistry.resolve(tax[0], tax[1], tax[2], tax[3]);
      if (def === null) return `"${name}" is not in the definition registry (unseeded?).`;
      let i = 1;
      let x: number;
      let y: number;
      if (args.length > i + 1 && Number.isFinite(Number(args[i])) && Number.isFinite(Number(args[i + 1]))) {
        x = Number(args[i]);
        y = Number(args[i + 1]);
        i += 2;
      } else {
        const r = this.panel.canvas.getBoundingClientRect();
        const w = this.panel.view.screenToWorld(r.width / 2, r.height / 2);
        x = Math.floor(w.x / SQUARE);
        y = Math.floor(w.y / SQUARE);
      }
      let body = 0;
      let head = 0;
      for (; i + 1 < args.length; i += 2) {
        const v = Number(args[i + 1]);
        if (!Number.isInteger(v) || v < 0 || v > 15) return usage; // the u4 lane is the hard bound
        if (args[i] === "body") body = v;
        else if (args[i] === "head") head = v;
        else return usage;
      }
      // Two-part kinds mint their PART entries (payload opcode 1: header `1<<16 | 2`,
      // slot, def-with-chosen-variant); single-part kinds mint bare like the wolf.
      const slots = (c.moverParts(objectId) as unknown[]).length;
      const words: number[] = [];
      if (slots >= 2) {
        const variantDef = (v: number) => ((def & ~0xf) | v) >>> 0;
        words.push((1 << 16) | 2, 0, variantDef(body));
        words.push((1 << 16) | 2, 1, variantDef(head));
      }
      // PROMOTE(1) CREATE(3) def pos count payload… — the npc's own mint shape.
      const program = new Uint32Array([1, 3, def, tileToPosition(x, y), words.length, ...words]);
      this.ctx.client.queue(program);
      return `Spawning ${name} at (${x}, ${y})${slots >= 2 ? ` body ${body} head ${head}` : ""} — [${[...program].join(", ")}]`;
    });

    // No server-side pause verb in the rebuild yet.
    this.chat.registerCommand("pause", () => "Pause isn't wired in the rebuild yet.");
    this.chat.registerCommand("unpause", () => "Pause isn't wired in the rebuild yet.");

    // RT preview / overlay / shadow / spikes — infra pending (W4f); report rather than error.
    for (const [name, msg] of Object.entries(PENDING_W4F)) {
      this.chat.registerCommand(name, () => msg);
    }
  }

  /** Replay the URL-passed commands (parsed in {@link onEnter}) through the chat, once. */
  private runUrlCommands(): void {
    for (const c of this.urlCommands) {
      if (!this.chat.execCommand(c.name, c.args)) {
        console.warn(`[WorldScene] URL command /${c.name} is not a known command — skipped.`);
      }
    }
  }

  // ── build placement mode (build-walls P2, D4) ─────────────────────────────────────

  /** Enter wall placement for `entry` (the panel's icon click). Selection clicks suspend;
   *  left-drag forms the rect; right-click exits. */
  private enterBuildMode(entry: BuildEntry): void {
    this.buildMode = entry;
    this.buildDragStart = null;
    this.panel.canvas.style.cursor = "crosshair";
  }

  private exitBuildMode(): void {
    this.buildMode = null;
    this.buildDragStart = null;
    this.panel.view.setBlueprint([]);
    this.panel.canvas.style.cursor = "";
    this.buildPanel?.setActive(null);
  }

  private tileAt(cx: number, cy: number): { x: number; y: number } {
    const w = this.clientToWorld(cx, cy);
    return { x: Math.floor(w.x / SQUARE), y: Math.floor(w.y / SQUARE) };
  }

  /** The drag rect's PERIMETER tiles (the walls a release will order). */
  private buildPerimeter(ax: number, ay: number, bx: number, by: number): Array<{ x: number; y: number }> {
    const x0 = Math.min(ax, bx), x1 = Math.max(ax, bx);
    const y0 = Math.min(ay, by), y1 = Math.max(ay, by);
    const out: Array<{ x: number; y: number }> = [];
    for (let x = x0; x <= x1; x++) {
      for (let y = y0; y <= y1; y++) {
        if (x === x0 || x === x1 || y === y0 || y === y1) out.push({ x, y });
      }
    }
    return out;
  }

  /** Rebuild the blueprint preview for the current drag — the blueprint linked atlas,
   *  variant-aware against the PREVIEW SHAPE itself (D1 CPU-side; client-only). */
  private syncBuildPreview(cx: number, cy: number): void {
    if (!this.buildMode || !this.buildDragStart) return;
    const cur = this.tileAt(cx, cy);
    const per = this.buildPerimeter(this.buildDragStart.x, this.buildDragStart.y, cur.x, cur.y);
    const shape = new Set<number>(per.map((t) => ((t.x & 0xffff) << 16) | (t.y & 0xffff)));
    const inShape = (x: number, y: number): boolean => shape.has(((x & 0xffff) << 16) | (y & 0xffff));
    const stem = `${blueprintStemFor(this.buildMode.stem)}/l`;
    const tiles: BlueprintTile[] = per.map((t) => {
      const mask = (inShape(t.x, t.y - 1) ? MASK_N : 0) | (inShape(t.x + 1, t.y) ? MASK_E : 0)
                 | (inShape(t.x, t.y + 1) ? MASK_S : 0) | (inShape(t.x - 1, t.y) ? MASK_W : 0);
      const f = this.ctx.textureResolver.resolve(stem, "albedo", linkedCell(mask)).frame;
      return { tileX: t.x, tileY: t.y, frame: f ? { source: f.source, x: f.x, y: f.y, w: f.w, h: f.h } : null };
    });
    this.panel.view.setBlueprint(tiles);
  }

  // ── drag-to-pan (through the bridge, so zone subscriptions follow) ────────────────
  // ui-select P0: pan rides the MIDDLE button (1) only — left (0) is selection's, right (2)
  // is the move order's. preventDefault on middle stops the browser's autoscroll widget.
  // build-walls P2 (D4): an active placement mode reroutes left (start the rect) and right
  // (exit the mode); middle-pan stays live.
  // input-rework F1 (`design/input-model.md`): RIGHT click = the ACTIVE selection (the
  // whole former left-click behavior); LEFT click = the context pie menu (resolved on UP
  // through the same slop test, so a future drag-box keeps its branch); middle = pan.
  // The PieMenu dismisses itself on any other input (document-capture listeners).
  private readonly onPointerDown = (e: PointerEvent): void => {
    if (this.buildMode && e.button === 0) {
      this.buildDragStart = this.tileAt(e.clientX, e.clientY);
      this.syncBuildPreview(e.clientX, e.clientY);
      return;
    }
    if (this.buildMode && e.button === 2) {
      this.exitBuildMode();
      return;
    }
    if (e.button === 0) {
      this.clickId = e.pointerId;
      this.clickX = e.clientX;
      this.clickY = e.clientY;
      return;
    }
    if (e.button === 2) {
      this.selectAt(e.clientX, e.clientY);
      return;
    }
    if (e.button !== 1) return;
    e.preventDefault();
    this.dragId = e.pointerId;
    this.lastClientX = e.clientX;
    this.lastClientY = e.clientY;
    this.panel.canvas.setPointerCapture(e.pointerId);
  };

  private readonly onPointerMove = (e: PointerEvent): void => {
    if (this.buildMode && this.buildDragStart) this.syncBuildPreview(e.clientX, e.clientY);
    if (this.dragId !== e.pointerId) return;
    // RENDER SCALE, not logical zoom. A screen-px drag is `1 / renderScale` world px — that is the
    // actual world→screen factor the projection uses. Dividing by `camera.zoom` here made the ground
    // slide at the wrong rate under the cursor by exactly the cover-fit factor (0.5553 on a 1862-px
    // panel, so ~1.8× too far per pixel) once the cover fit landed.
    const z = this.panel.view.camera.renderScale;
    // Drag right → world slides right under the cursor → anchor moves left.
    this.bridge.moveBy((this.lastClientX - e.clientX) / z, (this.lastClientY - e.clientY) / z);
    this.lastClientX = e.clientX;
    this.lastClientY = e.clientY;
  };

  private readonly onPointerUp = (e: PointerEvent): void => {
    // build-walls P2/P3: releasing the left button in a build drag ISSUES the wall order
    // (start tile, end tile, the kind's def id) and clears the client-only preview. The
    // mode STAYS active for the next rect; right-click exits.
    if (this.buildMode && this.buildDragStart && e.button === 0) {
      const start = this.buildDragStart;
      const end = this.tileAt(e.clientX, e.clientY);
      this.buildDragStart = null;
      this.panel.view.setBlueprint([]);
      this.bridge.buildWall(start.x, start.y, end.x, end.y, this.buildMode.defId);
      return;
    }
    // A pending left click resolves on UP: within the slop it opens the CONTEXT MENU
    // (input-rework F1); past it, it was a drag (the future drag-box claims this branch).
    if (this.clickId === e.pointerId) {
      this.clickId = null;
      if (Math.abs(e.clientX - this.clickX) <= CLICK_SLOP_PX && Math.abs(e.clientY - this.clickY) <= CLICK_SLOP_PX) {
        this.openContextMenu(e.clientX, e.clientY);
      }
      return;
    }
    if (this.dragId !== e.pointerId) return;
    this.dragId = null;
    try {
      this.panel.canvas.releasePointerCapture(e.pointerId);
    } catch {
      /* capture may already be gone */
    }
  };

  private readonly onContextMenu = (e: Event): void => e.preventDefault();

  /** Client (css) coords → world px, via the canonical camera mapping. */
  private clientToWorld(cx: number, cy: number): { x: number; y: number } {
    const r = this.panel.canvas.getBoundingClientRect();
    return this.panel.view.screenToWorld(cx - r.left, cy - r.top);
  }

  /** ui-select P1: rebuild the outline set from the selection — per frame, because a selected
   *  pawn's prim moves. Sprite mode with the surface frame (silhouette outline); box mode for
   *  tiles and anything whose surface hasn't resolved. */
  private syncOutlines(): void {
    const items: OutlineItem[] = [];
    for (const s of this.selection.all) {
      if (s.kind === "tile") {
        items.push({ x: s.x * SQUARE, y: s.y * SQUARE, w: SQUARE, h: SQUARE, mode: "box" });
        continue;
      }
      // human-pawns-redux P1 (I4): a pawn outlines EVERY part (a human's head with its
      // body); things stay single-prim.
      const prims = [];
      if (s.kind === "pawn") {
        for (const id of this.moverLayer.partPrimIdsOf(s.entity)) {
          const p = this.panel.view.warmGetPrim(id);
          if (p) prims.push(p);
        }
      } else {
        const p = this.panel.view.coldGetPrim(s.primId);
        if (p) prims.push(p);
      }
      // despawned/streamed out — outline simply absent until it returns
      for (const prim of prims) {
        let frame;
        if (prim.textureName) {
          const surf = this.ctx.textureResolver.resolve(prim.textureName, "surface", prim.cell)?.frame;
          if (surf) frame = { source: surf.source, x: surf.x, y: surf.y, w: surf.w, h: surf.h };
        }
        items.push({ x: prim.x, y: prim.y, w: prim.width, h: prim.height, mode: "sprite", frame, flip: prim.flipX });
      }
    }
    this.panel.view.setOutlines(items);
  }

  /** ui-select P0 (D2): the left-click hit test — topmost PAWN, else cold THING by tight
   *  silhouette box, else the TILE under the cursor. Single-click = replace; the drag-box
   *  pass calls the same model with many refs. */
  private selectAt(cx: number, cy: number): void {
    const w = this.clientToWorld(cx, cy);
    const pawn = this.moverLayer.pawnAt(w.x, w.y);
    let sel: Selection;
    if (pawn) {
      sel = { kind: "pawn", entity: pawn.entity };
    } else {
      const thing = this.panel.view.thingAt(w.x, w.y);
      sel = thing
        ? { kind: "thing", primId: thing.primId }
        : { kind: "tile", x: Math.floor(w.x / SQUARE), y: Math.floor(w.y / SQUARE) };
    }
    this.selection.replace(sel);
  }

  /** input-rework F1/F4/F5: the LEFT-click context pie menu — what the ACTIVE pawn can do
   *  with the clicked object. Availability comes WHOLE from the wasm `tileMenuOptions`
   *  (predicates + the location rule — the same questions the worker enforces); an empty
   *  set opens NOTHING (F7). Thing targets offer nothing this stream (I9 — no thing
   *  carries interactions, and prims don't expose their kind yet); pawn targets are the
   *  recorded social successor. */
  private openContextMenu(cx: number, cy: number): void {
    this.pieMenu.close();
    const primary = this.selection.primary;
    if (!primary || primary.kind !== "pawn") return;
    const actor = primary.entity;
    const info = this.moverLayer.pawnInfo(actor);
    if (!info) return;
    const w = this.clientToWorld(cx, cy);
    if (this.moverLayer.pawnAt(w.x, w.y) || this.panel.view.thingAt(w.x, w.y)) return;
    const tileX = Math.floor(w.x / SQUARE);
    const tileY = Math.floor(w.y / SQUARE);
    const defId = this.bridge.tileDefAt(tileX, tileY);
    if (defId === 0) return; // unstreamed ground — nothing to offer
    const d = this.ctx.client.ticDelta(0);
    if (d === null) return;
    const now = ((Math.floor(d) % 0x10000) + 0x10000) % 0x10000;
    const payload = this.moverLayer.pawnPayload(actor) ?? new Uint32Array(0);
    const needs = this.moverLayer.pawnNeeds(actor);
    // lumberjack F2: the filter takes the Chebyshev pawn↔clicked-cell distance and runs
    // the SHARED location_in_range — "on" = 0, "adjacent" ≤ 1 (inclusive).
    const cheb = Math.max(Math.abs(info.tileX - tileX), Math.abs(info.tileY - tileY));
    const options = getContent().tileMenuOptions(defId, payload, needs, now, cheb) as unknown as PieMenuOption[];
    this.pieMenu.open(cx, cy, options, (o) => this.fireOption(o, actor, tileX, tileY));
  }

  /** input-rework F5: bind an option's input SIGNATURE by the reserved vocabulary and
   *  queue the `EXECUTE_INTERACTION` event. An unbindable name refuses loudly — the wasm
   *  filter should never have offered it. */
  private fireOption(o: PieMenuOption, actor: number, tileX: number, tileY: number): void {
    const inputs = new Uint32Array(o.inputs.length);
    for (let i = 0; i < o.inputs.length; i++) {
      const name = o.inputs[i];
      if (name === "pawn") inputs[i] = actor;
      else if (name === "destination") inputs[i] = tileToPosition(tileX, tileY);
      else if (name === "amount") {
        const f = new Float32Array(1);
        f[0] = o.magnitude;
        inputs[i] = new Uint32Array(f.buffer)[0];
      } else {
        console.warn(`pie menu: unbindable input '${name}' on ${o.name}`);
        return;
      }
    }
    this.ctx.client.queue(composeInteraction(o.reference, inputs));
  }

  // ── scroll-to-zoom (about the cursor, through the bridge) ─────────────────────────
  // Continuous, per-event zoom (matching pixijs): every wheel tick applies a smooth
  // factor 2^(-deltaY/WHEEL_OCTAVE) — no discrete accumulator.
  private readonly onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    const r = this.panel.canvas.getBoundingClientRect();
    const factor = Math.pow(2, -e.deltaY / WHEEL_OCTAVE);
    const anchor = this.panel.view.camera.zoomAt(e.clientX - r.left, e.clientY - r.top, factor);
    if (anchor) this.bridge.zoomTo(anchor.x, anchor.y, this.panel.view.camera.zoom);
  };
}
