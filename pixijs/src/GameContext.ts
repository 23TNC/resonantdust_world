//! The app-wide context handed to scenes and panels. In this fresh `pixijs`
//! client it carries only the UI-framework surface plus the stub client — the
//! game-manager fields (textures/cards/definitions/layout/input/…) are added
//! back as each subsystem is rebuilt on top.

import type { Application } from "pixi.js";
import type { SceneManager } from "./scenes/SceneManager";
import type { PanelManager } from "./ui/panels/PanelManager";
import type { PanelTaskbar } from "./ui/dom/PanelTaskbar";
import type { UiEditMode } from "./ui/dom/UiEditMode";
import type { WasmClient } from "./client/WasmClient";
import type { Content } from "./client/wasm";
import type { DrawCallCounter } from "./debug/DrawCallCounter";
import type { LogManager } from "./game/panels/chat/LogManager";
import type { DebugPanel } from "./game/panels/titlebar/DebugPanel";
import type { TextureResolver } from "./textures";

export interface GameContext {
  readonly app: Application;
  readonly scenes: SceneManager;

  /** Bridge to the client core (gate connection + login + world state). A no-op
   *  stub until the networking layer is rebuilt — see {@link WasmClient}. */
  readonly client: WasmClient;

  /** The bottom (primary) panel taskbar. */
  readonly taskbar: PanelTaskbar;
  /** The top taskbar (title-bar tools). */
  readonly topTaskbar: PanelTaskbar;
  /** UI layout-edit toggle (drag/resize panels). */
  readonly uiEditMode: UiEditMode;

  /** Per-frame GL draw-call counter (patched at boot); read by the debug panel. */
  readonly drawCalls: DrawCallCounter;

  /** The three-tier texture resolver (master → preview → geo). Owns the per-LOD
   *  atlas pools and the all-purpose `white` fill (baked into the preview pool).
   *  The viewport bakes named prims through it; loads upgrade in place. */
  readonly textureResolver: TextureResolver;

  /** The DSL content runtime (def_id → colour), fetched at boot from the gateway
   *  and hot-swapped when the gate's corpus version moves. Mutable: content-reload
   *  replaces this handle (see `contentBoot.onContentReloaded`); consumers that
   *  cache it (the world bridge) also subscribe to reload so they repaint. */
  content: Content;

  /** The stats HUD — late-bound after context creation. */
  debugPanel?: DebugPanel;

  /** Open-panel registry — set once the active scene installs it. */
  panels: PanelManager | null;

  /** Client-only flavor-text log feed (the chat panel's "logs" tab); null until
   *  a scene installs it. Game systems push local event blurbs here. */
  logs: LogManager | null;
}
