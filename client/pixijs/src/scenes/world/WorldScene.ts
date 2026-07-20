import { Scene } from "../Scene";
import type { GameContext } from "../../GameContext";
import { PanelManager } from "../../ui/panels/PanelManager";
import { ChatPanel } from "../../game/panels/chat/ChatPanel";
import { LogManager } from "../../game/panels/chat/LogManager";
import { LayoutNode } from "../../game/layout/LayoutNode";
import { ViewportPanel } from "../../game/viewport/ViewportPanel";
import { RtPanel } from "../../game/panels/rt/RtPanel";
import { WorldBridge } from "../../game/world/WorldBridge";
import { MoverLayer } from "../../game/world/MoverLayer";
import { onContentReloaded, getContent } from "../../game/definitions/contentBoot";
import { parseUrl, type UrlCommand } from "../../debug/urlParams";
import { SQUARE } from "../../game/viewport/squareMath";

/** Accumulated wheel `deltaY` that halves or doubles the zoom (one LOD octave). */
const WHEEL_OCTAVE = 500;

/** Pointer travel (CSS px) below which a press-release counts as a click (a move
 *  command), not a drag (a pan). */
const CLICK_SLOP = 4;

/**
 * The world scene — the post-login game surface. In the old client this hosted
 * the viewport, card drag-drop, input routing, and the details/RT panels. None
 * of those subsystems have been rebuilt yet, so this is trimmed to a **panel
 * host**: it installs the per-scene {@link PanelManager} + {@link LogManager}
 * and opens the {@link ChatPanel}.
 *
 * The app-global title-bar tools (settings / debug / the per-panel settings
 * popup) are built at boot in `main.ts` and live across every scene; this scene
 * only owns the world-scoped chat panel.
 *
 * This is the seam the real world scene rebuilds into — viewport, layout/input
 * managers, and the details/RT panels slot back in here as they return.
 */
export class WorldScene extends Scene {
  private ctx!: GameContext;
  /** World chat — a taskbar-pinned DOM panel (general feed + client-only logs
   *  tab + send input). Server messages would stream through `ctx.client.onChat`
   *  once the client core is rebuilt; today the feed stays empty. */
  private chat!: ChatPanel;
  private unsubPaused: (() => void) | null = null;
  /** Transform-free Pixi layer the PixiPanel chrome + content attach to. Nothing
   *  in this fresh client instantiated a `PixiPanel` before, so the world scene
   *  owns the one layout root + drives its per-frame layout pass. */
  private panelLayer!: LayoutNode;
  /** The world viewport (albedo-map renderer in a PixiPanel). */
  private viewport!: ViewportPanel;
  /** Render-texture preview panel (`/showRT`), or null while closed. Ticked each
   *  frame so its live thumbnails track the viewport's composites. */
  private rt: RtPanel | null = null;
  /** Wiring from the world client's zone stream into the viewport, and the
   *  viewport camera into the client's anchor. */
  private bridge!: WorldBridge;
  /** The tick pipeline's mobile entities (pawns — the wolves): synced into the viewport's
   *  WARM cache as prims (composited over the cold world), not a separate overlay. */
  private moverLayer!: MoverLayer;
  /** Unsubscribe from content hot-swaps; called on scene exit. */
  private contentUnsub: (() => void) | null = null;
  /** Commands parsed from the URL query (see `debug/urlParams`) — replayed ONCE after the scene
   *  is wired, so a `?ambient=1.5` param runs the same handler as typing `/ambient 1.5`. */
  private urlCommands: UrlCommand[] = [];
  /** Drag-to-pan state: pointer id + last client-px position while dragging. */
  private dragId: number | null = null;
  private dragX = 0;
  private dragY = 0;
  /** Where the current press started, and whether it has crossed {@link CLICK_SLOP}
   *  into a drag — so a release below the slop fires a move command instead. */
  private downX = 0;
  private downY = 0;
  private dragged = false;
  /** Which mouse button started the press — left (0) = move, right (2) = interact. */
  private downButton = 0;
  /** Bound pointer handlers (so they can be removed on exit). */
  private readonly onPointerDown = (e: PointerEvent): void => {
    this.dragId = e.pointerId;
    this.dragX = e.clientX;
    this.dragY = e.clientY;
    this.downX = e.clientX;
    this.downY = e.clientY;
    this.downButton = e.button;
    this.dragged = false;
  };
  /** Suppress the browser context menu so a right-click can mean "interact". */
  private readonly onContextMenu = (e: Event): void => e.preventDefault();
  private readonly onPointerMove = (e: PointerEvent): void => {
    if (this.dragId !== e.pointerId) return;
    if (!this.dragged && Math.hypot(e.clientX - this.downX, e.clientY - this.downY) > CLICK_SLOP) {
      this.dragged = true;
    }
    if (!this.dragged) return; // still within the click slop — don't pan yet
    // Drag right → world slides right under the cursor → anchor moves left. A screen-px
    // drag is `1/zoom` world px, so divide the delta by the zoom.
    const z = this.viewport.view.zoom;
    this.bridge.moveBy((this.dragX - e.clientX) / z, (this.dragY - e.clientY) / z);
    this.dragX = e.clientX;
    this.dragY = e.clientY;
  };
  /** Scroll wheel → zoom about the cursor. Each `WHEEL_OCTAVE` of accumulated
   *  `deltaY` halves/doubles the zoom; the world point under the cursor stays put. */
  private readonly onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    const r = this.ctx.app.canvas.getBoundingClientRect();
    const factor = Math.pow(2, -e.deltaY / WHEEL_OCTAVE);
    const anchor = this.viewport.view.zoomAt(e.clientX - r.left, e.clientY - r.top, factor);
    if (anchor) this.bridge.zoomTo(anchor.x, anchor.y, this.viewport.view.zoom);
  };
  private readonly onPointerUp = (e: PointerEvent): void => {
    if (this.dragId !== e.pointerId) return;
    // A press-release that never crossed the slop is a click: left → move the session's own
    // pawn to the clicked tile, right → place (spawn/relocate) it there.
    if (!this.dragged) {
      if (this.downButton === 2) this.tileClick(e, "place");
      else this.tileClick(e, "move");
    }
    this.dragId = null;
  };
  /** Convert a click to a global tile and drive the session's own pawn: `move` (left click) sends
   *  a `MOVE_TO`, `place` (right click) a `PROMOTE_STATE`+`PLACE` (spawn or relocate). Both queue an
   *  action program via the edge. */
  private tileClick(e: PointerEvent, kind: "move" | "place"): void {
    const r = this.ctx.app.canvas.getBoundingClientRect();
    const w = this.viewport.view.screenToWorld(e.clientX - r.left, e.clientY - r.top);
    const tx = Math.floor(w.x / SQUARE);
    const ty = Math.floor(w.y / SQUARE);
    if (kind === "place") this.ctx.client.placeSelf(tx, ty);
    else this.ctx.client.moveSelf(tx, ty);
  }
  /** End the drag when the cursor leaves the canvas: the matching `pointerup`
   *  fires off-canvas where we don't hear it, so without this we'd keep panning
   *  with no button held once the cursor re-enters. */
  private readonly onPointerLeave = (): void => {
    this.dragId = null;
  };

  onEnter(ctx: GameContext): void {
    this.ctx = ctx;
    ctx.panels = new PanelManager();
    // Client-only flavor-text feed backing the chat panel's "logs" tab. Created
    // before ChatPanel so its constructor can subscribe; game systems push here.
    ctx.logs = new LogManager();

    // Pixi panel layer: a context-bearing LayoutNode at the stage origin (no
    // transform), so PixiPanels can hand DOMRect (CSS px) coords straight to it.
    this.panelLayer = new LayoutNode();
    this.panelLayer.setContext(ctx);
    this.root.addChild(this.panelLayer.container);

    // The world viewport, opened into the panel layer.
    this.viewport = new ViewportPanel(ctx, this.panelLayer);
    this.viewport.open();

    // URL query params are replayed as chat commands once this scene is wired (see
    // `debug/urlParams` + `runUrlCommands`). The camera-jump one (`?focus=x,y` → `/focus`) also
    // seeds the INITIAL anchor here, so the first subscription is already at the target tile (no
    // origin flash / wasted subscription) — the replayed `/focus` then re-applies it as a no-op.
    this.urlCommands = parseUrl().commands;
    const focusCmd = this.urlCommands.find((c) => c.name === "focus");
    const startX = focusCmd ? Number(focusCmd.args[0]) || 0 : 0;
    const startY = focusCmd ? Number(focusCmd.args[1]) || 0 : 0;

    // Wire the client's zone stream into the viewport and start the anchor — login has
    // completed by the time this scene enters, so the first anchor immediately subscribes
    // the zones around it (at the origin, or the `?focus=x,y` tile).
    this.bridge = new WorldBridge(ctx.client, ctx.content, this.viewport.view, ctx.textureResolver.white, ctx.textureResolver);
    this.bridge.start(startX, startY);
    // Mobile entities (pawns) from the shard `state` stream — the bot-driven wolves,
    // drawn as an overlay above the world.
    this.moverLayer = new MoverLayer(ctx.client, ctx.content, this.viewport.view);

    // Repaint live zones when the gate hot-swaps the corpus. `getContent()` is the
    // freshly-swapped bundle (independent of listener order vs `ctx.content`).
    this.contentUnsub = onContentReloaded(() => {
      const c = getContent();
      this.bridge.setContent(c);
      this.moverLayer.setContent(c);
    });

    // Drag-to-pan: move the anchor as the user drags the canvas (which streams
    // zones in/out via the hysteresis ladder).
    const canvas = ctx.app.canvas;
    canvas.addEventListener("pointerdown", this.onPointerDown);
    canvas.addEventListener("pointermove", this.onPointerMove);
    canvas.addEventListener("pointerup", this.onPointerUp);
    canvas.addEventListener("pointercancel", this.onPointerUp);
    canvas.addEventListener("pointerleave", this.onPointerLeave);
    canvas.addEventListener("contextmenu", this.onContextMenu);
    // `passive: false` so the handler can preventDefault the page scroll.
    canvas.addEventListener("wheel", this.onWheel, { passive: false });

    // Taskbar-pinned DOM panel. The client core would subscribe `chat_messages`
    // on login and stream them through `ctx.client.onChat`; the stub never fires,
    // so the general feed is empty until the networking layer returns. Slash
    // commands (parsed locally) still work — an unknown one echoes a system line.
    this.chat = new ChatPanel(ctx);
    this.chat.open();

    this.registerCommands();
    this.unsubPaused = this.ctx.client.onPaused((paused) => {
      this.chat.systemLine(paused ? "⏸ Simulation paused (tic frozen)." : "▶ Simulation resumed.");
    });

    // Replay the URL query as chat commands, once, now that the viewport/bridge/chat are wired.
    // Auto-login (`?user`) or manual login both reach here — the params run either way.
    this.runUrlCommands();
  }

  /** Register every chat command against the chat panel. Each is also reachable from the URL query
   *  (`debug/urlParams` → {@link runUrlCommands}), so `?grid=1` and typing `/grid 1` hit the same
   *  handler. Debug commands sit alongside the panel/pause ones. */
  private registerCommands(): void {
    const view = () => this.viewport.view;

    // `/showRT` — open (or re-focus) the render-texture preview for the viewport.
    this.chat.registerCommand("showRT", () => this.showRenderTextures());

    // `/overlayRT <channel>` — draw one G-buffer composite over the lit viewport (world-aligned,
    // full size). Toggles: the same channel again turns it off. Empty/default values (flat normal,
    // black lightmap/shadow/depth) read through to the lit scene beneath.
    this.chat.registerCommand("overlayRT", (args) => this.overlayRenderTexture(args));

    // `/grid [0|1|2|3]` — overlay the debug grid at a DETAIL LEVEL: 1 = region + zone + tile,
    // 2 = region + zone, 3 = region only, 0 = off. No arg TOGGLES (off ⇄ full detail).
    const gridLabel = ["Debug grid off.", "Debug grid: region + zone + tile.", "Debug grid: region + zone.", "Debug grid: region."];
    this.chat.registerCommand("grid", (args) => {
      let level: number;
      if (args.length === 0) {
        level = view().debugGrid > 0 ? 0 : 1; // toggle: off ⇄ full detail
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

    // `/focus <tileX> <tileY>` — jump the camera centre to a tile (the `?focus=x,y` URL param).
    this.chat.registerCommand("focus", (args) => {
      const x = Number(args[0]);
      const y = Number(args[1]);
      if (!Number.isFinite(x) || !Number.isFinite(y)) return "Usage: /focus <tileX> <tileY>";
      this.bridge.setAnchor(x * SQUARE, y * SQUARE);
      return `Camera focused on tile (${x}, ${y}).`;
    });

    // `/zoom <level>` — set the ABSOLUTE zoom (screen px per world px), holding the viewport centre.
    // Clamps to the LOD range: 1 = native 64px tiles, 2 = 2× in, 0.5 = 2× out (the `?zoom=` param).
    this.chat.registerCommand("zoom", (args) => {
      const z = Number(args[0]);
      if (!Number.isFinite(z) || z <= 0) return "Usage: /zoom <level>  (1 = native, 2 = 2× in, 0.5 = out)";
      const anchor = view().setZoom(z);
      if (anchor) this.bridge.zoomTo(anchor.x, anchor.y, view().zoom);
      return `Zoom set to ${view().zoom}×.`;
    });

    // `/shadowcast` (shadow-cast experiment) — toggle: 5 cold lights in the (100,50) zone cast shadows
    // from in-radius prims into a ping-pong bitfield RT (one bit each), shown as 5 colours; one light
    // moves per second, re-casting only it. Proves shadow-casting into a bitfield + incremental updates.
    this.chat.registerCommand("shadowcast", () => {
      const on = view().toggleShadowCast();
      return on ? "Shadow-cast ON (5 lights, 5 colours; one moves/sec)." : "Shadow-cast OFF.";
    });

    // `/pause` + `/unpause` (debug) — no server-side pause in the rebuild yet (the master's
    // metronome has no freeze verb), so these report unavailability rather than silently do
    // nothing. `onPaused` stays wired (it simply never fires) for when a freeze verb returns.
    this.chat.registerCommand("pause", () => "Pause isn't wired in the rebuild yet.");
    this.chat.registerCommand("unpause", () => "Pause isn't wired in the rebuild yet.");
  }

  /** Replay the URL-passed commands (parsed in {@link onEnter}) through the chat panel, once. Each
   *  runs the exact handler a typed `/command` would; unrecognized params are skipped silently
   *  (they may be unrelated query keys), and a bad arg surfaces the command's own usage line. */
  private runUrlCommands(): void {
    for (const c of this.urlCommands) {
      if (!this.chat.execCommand(c.name, c.args)) {
        console.warn(`[WorldScene] URL command /${c.name} is not a known command — skipped.`);
      }
    }
  }

  /** `/showRT` chat command — open (or re-focus) the render-texture preview panel
   *  for the world viewport. A singleton (keyed `showRT` in the PanelManager); a
   *  repeat just re-focuses the existing one. */
  private showRenderTextures(): string {
    const panels = this.ctx.panels;
    if (!panels) return "Render textures unavailable.";
    panels.ensure("showRT", () => {
      const vp = this.viewport;
      const panel = new RtPanel({
        parent: this.panelLayer,
        label: "RT · Game View",
        storageKey: "showRT",
        source: () => ({ aspect: vp.aspect(), channels: vp.renderTextures() }),
        taskbar: this.ctx.taskbar,
        uiEditMode: this.ctx.uiEditMode,
      });
      panels.registerNode(panel.content, panel);
      this.rt = panel;
      panel.onDestroy(() => {
        if (this.rt === panel) this.rt = null;
      });
      return panel;
    });
    return "Showing render textures.";
  }

  /** `/overlayRT <channel>` chat command — toggle a render-texture composite as a full-viewport
   *  overlay over the lit world. No arg (or an unknown channel) echoes the valid channel list;
   *  a valid one switches the overlay to it, or turns it off if it's already showing. */
  private overlayRenderTexture(args: string[]): string {
    const names = this.viewport.view.overlayChannelNames();
    const name = args[0];
    if (!name) {
      const cur = this.viewport.view.overlayChannel;
      return `Usage: /overlayRT <channel>. ${cur ? `Showing ${cur}. ` : ""}Channels: ${names.join(", ")}.`;
    }
    if (!names.includes(name)) {
      return `Unknown channel "${name}". Channels: ${names.join(", ")}.`;
    }
    const now = this.viewport.view.setOverlay(name);
    return now ? `Overlaying ${now}.` : `Overlay off (${name}).`;
  }

  onResize(_width: number, _height: number): void {
    // Bounds in CSS px (matching DOMRect / stage coords) — used by layout hit-tests.
    this.panelLayer?.setBounds(0, 0, window.innerWidth, window.innerHeight);
  }

  override update(_deltaMS: number): void {
    // Lay out the Pixi panel chrome, then drive the viewport's bake + display.
    this.panelLayer?.layoutIfDirty();
    // The viewport drives the cold + warm bakes and the composited display; pawns are warm
    // prims (fed by the mover layer on state updates), so there's no per-frame mover work here.
    this.viewport?.tick();
    // Push the live zoom into the debug HUD (textures tab) — the viewport only
    // exists here, so `setStats` (scene-independent) can't read it.
    this.ctx.debugPanel?.setZoom(this.viewport.view.zoom);
    // RT preview re-reads the live composites (cheap unless a channel/size moved).
    this.rt?.tick();
  }

  onExit(): void {
    // Drop the pan handlers + the client/viewport wiring before tearing panels down.
    const canvas = this.ctx.app.canvas;
    canvas.removeEventListener("pointerdown", this.onPointerDown);
    canvas.removeEventListener("pointermove", this.onPointerMove);
    canvas.removeEventListener("pointerup", this.onPointerUp);
    canvas.removeEventListener("pointercancel", this.onPointerUp);
    canvas.removeEventListener("pointerleave", this.onPointerLeave);
    canvas.removeEventListener("contextmenu", this.onContextMenu);
    canvas.removeEventListener("wheel", this.onWheel);
    this.contentUnsub?.();
    this.contentUnsub = null;
    this.unsubPaused?.();
    this.unsubPaused = null;
    this.bridge.dispose();
    this.moverLayer.dispose();

    // Destroy PanelManager-registered panels (the RT preview) first, while their
    // parent layer is still alive, then the manually-owned panels + the layer.
    this.ctx.panels?.closeAll();
    this.viewport.destroy();
    this.chat.destroy();
    this.panelLayer.destroy();
    this.ctx.logs?.dispose();
    this.ctx.panels = null;
    this.ctx.logs = null;
  }
}
