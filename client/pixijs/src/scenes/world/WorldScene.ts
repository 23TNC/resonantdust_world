import { Scene } from "../Scene";
import type { GameContext } from "../../GameContext";
import { PanelManager } from "../../ui/panels/PanelManager";
import { ChatPanel } from "../../game/panels/chat/ChatPanel";
import { LogManager } from "../../game/panels/chat/LogManager";
import { LayoutNode } from "../../game/layout/LayoutNode";
import { ViewportPanel } from "../../game/viewport/ViewportPanel";
import { RtPanel } from "../../game/panels/rt/RtPanel";
import { WorldBridge } from "../../game/world/WorldBridge";
import { onContentReloaded, getContent } from "../../game/definitions/contentBoot";
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
  /** Unsubscribe from content hot-swaps; called on scene exit. */
  private contentUnsub: (() => void) | null = null;
  /** Drag-to-pan state: pointer id + last client-px position while dragging. */
  private dragId: number | null = null;
  private dragX = 0;
  private dragY = 0;
  /** Where the current press started, and whether it has crossed {@link CLICK_SLOP}
   *  into a drag — so a release below the slop fires a move command instead. */
  private downX = 0;
  private downY = 0;
  private dragged = false;
  /** Bound pointer handlers (so they can be removed on exit). */
  private readonly onPointerDown = (e: PointerEvent): void => {
    this.dragId = e.pointerId;
    this.dragX = e.clientX;
    this.dragY = e.clientY;
    this.downX = e.clientX;
    this.downY = e.clientY;
    this.dragged = false;
  };
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
    // A press-release that never crossed the slop is a click → move the mover to
    // the clicked tile.
    if (!this.dragged) this.moveToClick(e);
    this.dragId = null;
  };
  /** Convert a click to a global tile and send a move intent. The server pathfinds
   *  and commits the path; both players see the same authoritative motion. */
  private moveToClick(e: PointerEvent): void {
    const r = this.ctx.app.canvas.getBoundingClientRect();
    const w = this.viewport.view.screenToWorld(e.clientX - r.left, e.clientY - r.top);
    this.ctx.client.moveTo(Math.floor(w.x / SQUARE), Math.floor(w.y / SQUARE));
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

    // Wire the client's zone stream into the viewport and start the anchor at the
    // origin — login has completed by the time this scene enters, so the first
    // anchor immediately subscribes the zones around it.
    this.bridge = new WorldBridge(ctx.client, ctx.content, this.viewport.view, ctx.textureResolver.white, ctx.textureResolver);
    this.bridge.start();

    // Repaint live zones when the gate hot-swaps the corpus. `getContent()` is the
    // freshly-swapped bundle (independent of listener order vs `ctx.content`).
    this.contentUnsub = onContentReloaded(() => this.bridge.setContent(getContent()));

    // Drag-to-pan: move the anchor as the user drags the canvas (which streams
    // zones in/out via the hysteresis ladder).
    const canvas = ctx.app.canvas;
    canvas.addEventListener("pointerdown", this.onPointerDown);
    canvas.addEventListener("pointermove", this.onPointerMove);
    canvas.addEventListener("pointerup", this.onPointerUp);
    canvas.addEventListener("pointercancel", this.onPointerUp);
    canvas.addEventListener("pointerleave", this.onPointerLeave);
    // `passive: false` so the handler can preventDefault the page scroll.
    canvas.addEventListener("wheel", this.onWheel, { passive: false });

    // Taskbar-pinned DOM panel. The client core would subscribe `chat_messages`
    // on login and stream them through `ctx.client.onChat`; the stub never fires,
    // so the general feed is empty until the networking layer returns. Slash
    // commands (parsed locally) still work — an unknown one echoes a system line.
    this.chat = new ChatPanel(ctx);
    this.chat.open();

    // `/showRT` — open (or re-focus) the render-texture preview for the viewport.
    this.chat.registerCommand("showRT", () => this.showRenderTextures());
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

  onResize(_width: number, _height: number): void {
    // Bounds in CSS px (matching DOMRect / stage coords) — used by layout hit-tests.
    this.panelLayer?.setBounds(0, 0, window.innerWidth, window.innerHeight);
  }

  override update(_deltaMS: number): void {
    // Lay out the Pixi panel chrome, then drive the viewport's bake + display.
    this.panelLayer?.layoutIfDirty();
    this.viewport?.tick();
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
    canvas.removeEventListener("wheel", this.onWheel);
    this.contentUnsub?.();
    this.contentUnsub = null;
    this.bridge.dispose();

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
