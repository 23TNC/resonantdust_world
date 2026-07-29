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
import { LogManager } from "../../game/panels/chat/LogManager";
import { PanelManager } from "../../ui/panels/PanelManager";
import { SQUARE } from "../../game/viewport/squareMath";
import { SelectionModel, type Selection } from "../../game/world/SelectionModel";
import type { OutlineItem } from "../../game/viewport/outlineOverlay";
import { onContentReloaded, getContent } from "../../game/definitions/contentBoot";
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
  private contentUnsub: (() => void) | null = null;
  private urlCommands: UrlCommand[] = [];
  private dragId: number | null = null;
  private lastClientX = 0;
  private lastClientY = 0;
  /** ui-select P0 (D1): THE selection — outline/details/title-bar/move-order all read this. */
  readonly selection = new SelectionModel();
  /** A pending left CLICK (down seen, up not yet) — up within {@link CLICK_SLOP_PX} selects.
   *  The future drag-box replaces "discard on move" with "start the box" right here. */
  private clickId: number | null = null;
  private clickX = 0;
  private clickY = 0;
  /** ui-select P1 (D4): the cursor light's carrier prim (warm, hot; created on first move). */
  private cursorLightId: number | null = null;

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
    // the zoom so `radii()` sizes the zone subscription for the right lod. Driving either one alone looks
    // like a client bug: camera-only leaves the subscription sized for the previous lod (empty window
    // edges), bridge-only leaves the camera — and hence the lod — unchanged. Both mistakes cost real time
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
    // ui-select P1: a selected TILE shows its world coordinates in the panel's title bar
    // (the "selection" suffix — also visible on the taskbar entry while the bar is hidden).
    this.panel.setTitleSuffixResolver("selection", () => {
      const p = this.selection.primary;
      return p?.kind === "tile" ? `tile ${p.x}, ${p.y}` : null;
    });
    this.selection.subscribe(() => this.panel.refreshTitleSuffix());

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
    if (this.cursorLightId !== null) {
      this.panel?.view.warmRemovePrim(this.cursorLightId);
      this.cursorLightId = null;
    }
    this.chat?.destroy();
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

    // `/coldlights [tileX tileY]` — re-seed the 6 shadow-casting lights in a ring around a tile
    // (default 100 50, the design's zone). Shadows cast off the standing prims (things) in range.

    // `/shadows` — toggle the shadow pass.
    this.chat.registerCommand("shadows", () => (view().toggleShadows() ? "Shadows on." : "Shadows off."));

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

  // ── drag-to-pan (through the bridge, so zone subscriptions follow) ────────────────
  // ui-select P0: pan rides the MIDDLE button (1) only — left (0) is selection's, right (2)
  // is the move order's. preventDefault on middle stops the browser's autoscroll widget.
  private readonly onPointerDown = (e: PointerEvent): void => {
    if (e.button === 0) {
      this.clickId = e.pointerId;
      this.clickX = e.clientX;
      this.clickY = e.clientY;
      return;
    }
    if (e.button === 2) {
      this.issueMoveOrder(e.clientX, e.clientY);
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
    this.updateCursorLight(e.clientX, e.clientY); // ui-select P1 (D4) — every move, drag or not
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
    // A pending left click resolves on UP: within the slop it SELECTS; past it, it was a drag
    // (the future drag-box claims exactly this branch).
    if (this.clickId === e.pointerId) {
      this.clickId = null;
      if (Math.abs(e.clientX - this.clickX) <= CLICK_SLOP_PX && Math.abs(e.clientY - this.clickY) <= CLICK_SLOP_PX) {
        this.selectAt(e.clientX, e.clientY);
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
      let prim = null;
      if (s.kind === "pawn") {
        const id = this.moverLayer.primIdOf(s.entity);
        prim = id !== null ? this.panel.view.warmGetPrim(id) : null;
      } else {
        prim = this.panel.view.coldGetPrim(s.primId);
      }
      if (!prim) continue; // despawned/streamed out — outline simply absent until it returns
      let frame;
      if (prim.textureName) {
        const surf = this.ctx.textureResolver.resolve(prim.textureName, "surface", prim.cell)?.frame;
        if (surf) frame = { source: surf.source, x: surf.x, y: surf.y, w: surf.w, h: surf.h };
      }
      items.push({ x: prim.x, y: prim.y, w: prim.width, h: prim.height, mode: "sprite", frame, flip: prim.flipX });
    }
    this.panel.view.setOutlines(items);
  }

  /** ui-select P1 (D4): the cursor's small-radius HOT light — carried by a 1 px warm prim
   *  through the REAL placement path (`buildCasters` detects the carried light's move and
   *  routes the scoped dirty; hot class ⇒ cold maps never re-bake). No textureName ⇒ the
   *  prim never becomes a caster record — the light is its only presentation. */
  private updateCursorLight(cx: number, cy: number): void {
    const w = this.clientToWorld(cx, cy);
    const view = this.panel.view;
    if (this.cursorLightId === null) {
      this.cursorLightId = view.warmAddPrim({
        texture: view.white,
        x: w.x, y: w.y, width: 1, height: 1,
        tint: 0x000000, geoColor: 0x000000, zIndex: -1000, flipX: false,
        hot: true,
        light: {
          color: [1.0, 0.9, 0.7], intensity: 0.45, reach: 2 * SQUARE,
          emitterRadius: 8, height: 20 * (SQUARE / 16), castShadows: false, hot: true,
        },
      });
      return;
    }
    const p = view.warmGetPrim(this.cursorLightId);
    if (p) {
      p.x = w.x;
      p.y = w.y;
      view.warmRefreshPrim(this.cursorLightId); // re-bake the 1 px carrier (no stale dot)
    }
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

  /** ui-select P0 (D3): right click + a selected PAWN → a MOVE_TO from the pawn's location to
   *  the clicked tile, through core → edge (the npc's own verb). No selection → no-op. */
  private issueMoveOrder(cx: number, cy: number): void {
    const primary = this.selection.primary;
    if (!primary || primary.kind !== "pawn") return;
    const w = this.clientToWorld(cx, cy);
    this.bridge.moveEntity(primary.entity, Math.floor(w.x / SQUARE), Math.floor(w.y / SQUARE));
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
