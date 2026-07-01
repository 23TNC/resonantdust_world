//! `pixijs` entry point — boots the migrated scene + panel infrastructure.
//!
//! A PIXI Application, the SceneManager, the two panel taskbars + UI edit mode
//! the panel framework needs, and the app-global title-bar tools (settings +
//! debug HUD), wired into a minimal `GameContext`. Boots into the login scene;
//! a successful login hands off to the world scene (which opens the chat panel).
//! Game subsystems (content, viewport, cards) are rebuilt on top from here.

import { Application } from "pixi.js";
import { loadFonts } from "./assets/fonts";
import { SceneManager } from "./scenes/SceneManager";
import { LoginScene } from "./scenes/login/LoginScene";
import { PanelTaskbar } from "./ui/dom/PanelTaskbar";
import { UiEditMode } from "./ui/dom/UiEditMode";
import { PanelSettingsPopup } from "./ui/dom/PanelSettingsPopup";
import { DomPanel } from "./ui/dom/DomPanel";
import { DrawCallCounter } from "./debug/DrawCallCounter";
import { mountEnvOverlay } from "./debug/EnvOverlay";
import { SettingsMenu } from "./game/panels/titlebar/SettingsMenu";
import { DebugPanel } from "./game/panels/titlebar/DebugPanel";
import type { SyncStats } from "./game/panels/titlebar/DebugPanel";
import { SyncHistory } from "./game/panels/titlebar/syncHistory";
// Panel layout defaults are a pure client concern (DOM panel geometry) — NOT
// gate-served content, so they live in the view. This is the single source of
// truth the panel-settings "Copy All JSON" export pastes into.
import panelDefaults from "./content/panels/defaults.json";
import { WasmClient, type ClockStats, type CallStat, type SubStat } from "./client/WasmClient";
import { initWasm } from "./client/wasm";
import { loadContent } from "./game/definitions/contentBoot";
import { gatewayUrlFor, setCurrentEnvironment } from "./client/environments";
import { TextureManager } from "./textures";
import type { GameContext } from "./GameContext";

async function main(): Promise<void> {
  const app = new Application();
  await app.init({
    background: 0x101418,
    resizeTo: window,
    antialias: true,
    autoDensity: true,
    resolution: window.devicePixelRatio || 1,
    // Force WebGL — the draw-call counter patches the GL context, and the
    // (yet-to-return) lighting shaders are GLSL-only.
    preference: "webgl",
  });
  const host = document.getElementById("app");
  if (!host) throw new Error("#app element not found");
  host.appendChild(app.canvas);

  // Default the session env so the badge + debug HUD read sensibly before any
  // (future) login re-points it. Always-on env badge (top-left) so it's never
  // ambiguous which gate this session would talk to.
  setCurrentEnvironment("dev");
  mountEnvOverlay();

  // Content-shipped panel layout defaults — positions / sizes / toggle states
  // every `DomPanel` merges between its constructor opts and any persisted
  // localStorage state. Installed once, before any panel constructs. `_`-prefixed
  // keys (comment metadata) are stripped inside setPanelDefaults.
  DomPanel.setPanelDefaults(panelDefaults as Parameters<typeof DomPanel.setPanelDefaults>[0]);

  // Per-frame draw-call counter — patches the GL context so the debug panel can
  // report draw calls. Read + reset once per frame.
  const drawCalls = new DrawCallCounter();
  drawCalls.patch(app.renderer);

  await loadFonts();

  // Instantiate the Rust→wasm bundle (login + zone engine + DSL content) before
  // anything that constructs a `WorldClient` or `Content`.
  await initWasm();
  // The DSL tile content runtime, decoded from the embedded `content/` corpus —
  // the world scene queries it per zone to colour tiles.
  const content = loadContent();

  // Texture atlas pool — MaxRects-packed pages of rectangle textures. Bakes the
  // 32×32 WHITE fill primitive at construction; texture loading returns later.
  const textures = new TextureManager(app.renderer);

  const scenes = new SceneManager(app);

  // Panel framework: a bottom + top taskbar and the UI layout-edit toggle.
  const taskbar = new PanelTaskbar({ position: "bottom" });
  const topTaskbar = new PanelTaskbar({ position: "top" });
  const uiEditMode = new UiEditMode({
    reservedTop: PanelTaskbar.HEIGHT,
    reservedBottom: PanelTaskbar.HEIGHT,
  });
  // The per-panel settings popup (UI-edit-mode flyout) is app-global so every
  // scene's panels are editable. Its labels resolve via `panelStrings`, which
  // falls back to a bundled English copy before gate content loads.
  const settingsPopup = new PanelSettingsPopup();
  uiEditMode.settingsPopup = settingsPopup;

  // The client core bridge — owns the gateway round-trip + world-server
  // connection at login. Seeded with the dev gateway base; the login screen's
  // Server dropdown repoints it per attempt.
  const client = new WasmClient(gatewayUrlFor("dev"));

  const ctx: GameContext = {
    app,
    scenes,
    client,
    taskbar,
    topTaskbar,
    uiEditMode,
    drawCalls,
    textures,
    content,
    panels: null,
    logs: null,
  };
  scenes.setContext(ctx);

  // ── App-global title-bar tools ──────────────────────────────────────
  // Settings dropdown (⛯) + debug HUD (📊), pinned to the top taskbar and alive
  // across every scene — so the panel tools and debug info (fps / draw calls) are
  // reachable everywhere.
  const settingsMenu = new SettingsMenu(topTaskbar, uiEditMode);
  // Log Out: drop the world-server connection and return to the login screen.
  settingsMenu.onLogOut = () => {
    client.logout();
    scenes.change(new LoginScene()).catch((err) => {
      console.error("pixijs: return to login failed", err);
    });
  };
  // Clock-sync HUD source: the client seeds one snapshot from `login_ok` and a
  // running sync lands with the row stream; the sync tab reads "—" until login.
  const syncHistory = new SyncHistory();
  const debugPanel = new DebugPanel(topTaskbar, uiEditMode, syncHistory);
  ctx.debugPanel = debugPanel;
  // Map the client's clock diagnostics into the panel's `SyncStats` — fires once
  // per login (see `WasmClient.seedClock`).
  client.onClockStats((s: ClockStats) => {
    syncHistory.update(s.synced ? toSyncStats(s) : null);
  });
  // Per-reducer gateway-call tally → the debug panel's "calls" tab (inert until
  // call-stat accounting returns).
  client.onCallStats((stats: CallStat[]) => debugPanel.setCallStats(stats));
  // Per-table subscription tally → the debug panel's "subs" tab (inert until zone
  // subscriptions return).
  client.onSubStats((stats: SubStat[]) => debugPanel.setSubStats(stats));

  // Couple UI edit mode with the per-panel settings popup: entering edit mode
  // closes the Settings dropdown and opens the popup (bound to the last-focused
  // editable panel); closing the popup exits edit mode. Both directions are
  // idempotent-guarded, so the round-trip can't loop.
  uiEditMode.on((enabled) => {
    if (enabled) {
      settingsMenu.close();
      const target = DomPanel.lastFocusedEditTarget();
      if (target) settingsPopup.show(target);
    } else {
      settingsPopup.close();
    }
  });
  settingsPopup.onOpenChange((open) => {
    if (!open) uiEditMode.setEnabled(false);
  });

  // Drive the debug HUD every frame, scene-independent: frame-time → fps and the
  // GL draw-call tally. Atlas occupancy + clock-sync are omitted/empty until the
  // texture + networking subsystems return (`syncHistory.current()` is null under
  // the stub → the sync rows stay at "—").
  app.ticker.add((ticker) => {
    debugPanel.setStats(
      ticker.deltaMS,
      drawCalls.readAndReset(),
      undefined,
      syncHistory.current() ?? undefined,
    );
  });

  await scenes.change(new LoginScene());
}

/** Project the client core's `ClockStats` into the debug panel's `SyncStats`,
 *  filling the `Date.now()`-relative fields the core can't compute. Called once
 *  per login, when `s.synced` is set. */
function toSyncStats(s: ClockStats): SyncStats {
  const dateNowMs = Date.now();
  return {
    serverNowMs: s.serverNowMs,
    dateNowMs,
    offsetMs: s.serverNowMs - dateNowMs,
    captures: s.captures,
    bestOffsetMs: s.bestOffsetMs,
    worstOffsetMs: s.worstOffsetMs,
    deltaMs: s.deltaMs,
    clientLagMs: s.clientDelayMs,
    rttMs: s.rttMs,
    bestRttMs: s.bestRttMs,
    rttSamples: s.rttSamples,
    runningDeltaMs: s.runningDeltaMs,
    runningDelayMs: s.runningDelayMs,
  };
}

main().catch((e) => console.error("pixijs: boot failed", e));
