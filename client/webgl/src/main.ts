//! `webgl` entry point — boots the migrated scene + panel infrastructure on the
//! bespoke engine shell (no Pixi). An {@link App} (ticker + resize + mount host),
//! the SceneManager, the two panel taskbars + UI edit mode, and the app-global
//! title-bar tools (settings + video + debug HUD), wired into a `GameContext`.
//! Boots into the login scene; a successful login hands off to the world scene.
//! W3 milestone: login DOM form + WASM init + gateway connect, all on client/webgl.
//! The viewport (world render) lands in W4 — see docs/work/webgl-engine/todo.md.

import { App } from "./app/App";
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
import { VideoPanel } from "./game/panels/titlebar/VideoPanel";
import { DebugPanel } from "./game/panels/titlebar/DebugPanel";
import type { SyncStats, NowStats } from "./game/panels/titlebar/DebugPanel";
import { SyncHistory } from "./game/panels/titlebar/syncHistory";
// Panel layout defaults are a pure client concern (DOM panel geometry) — the
// single source of truth the panel-settings "Copy All JSON" export pastes into.
import panelDefaults from "./content/panels/defaults.json";
import { WasmClient, type ClockStats, type CallStat, type SubStatsSnapshot } from "./client/WasmClient";
import { initWasm } from "./client/wasm";
import {
  loadContent,
  reloadContent,
  startContentPolling,
  onContentReloaded,
  getContent,
} from "./game/definitions/contentBoot";
import { gatewayUrlFor, assetBaseFromServerUrl, setCurrentEnvironment } from "./client/environments";
import { TextureResolver, texturesRoot, persistStorage } from "./textures";
import type { GameContext } from "./GameContext";

async function main(): Promise<void> {
  const host = document.getElementById("app");
  if (!host) throw new Error("#app element not found");

  // The application shell — a frame ticker, a resize dispatch, and the `#app`
  // mount host. No global canvas: the viewport self-canvases in W4.
  const app = new App(host);

  // Default the session env so the badge + debug HUD read sensibly before login
  // re-points it. Always-on env badge (top-left) so it's never ambiguous which
  // gate this session would talk to.
  setCurrentEnvironment("dev");
  mountEnvOverlay();

  // Content-shipped panel layout defaults — installed once, before any panel
  // constructs. `_`-prefixed keys (comment metadata) are stripped inside.
  DomPanel.setPanelDefaults(panelDefaults as Parameters<typeof DomPanel.setPanelDefaults>[0]);

  // Per-frame draw-call counter — a stub in W3 (reports 0); wired to the engine
  // renderer when the viewport lands (W4).
  const drawCalls = new DrawCallCounter();
  drawCalls.patch();

  await loadFonts();

  // Instantiate the Rust→wasm bundle (login + zone engine + DSL content) before
  // anything that constructs a `WorldClient` or `Content`.
  await initWasm();
  // The DSL content runtime. Assets live on the world SERVER, unknown until
  // login, so boot from the build-time embed; `onLoggedIn` pulls the server's
  // corpus and hot-swaps it in.
  const content = await loadContent();

  // The three-tier texture resolver. In W3 it's a stub (no GPU): it records the
  // `/textures` root on login for the real atlas-backed resolver in W4. It starts
  // rootless (all geo) and `onLoggedIn` repoints it.
  const textureResolver = new TextureResolver(null, "");
  void persistStorage();

  const scenes = new SceneManager(app);

  // Panel framework: a bottom + top taskbar and the UI layout-edit toggle.
  const taskbar = new PanelTaskbar({ position: "bottom" });
  const topTaskbar = new PanelTaskbar({ position: "top" });
  const uiEditMode = new UiEditMode({
    reservedTop: PanelTaskbar.HEIGHT,
    reservedBottom: PanelTaskbar.HEIGHT,
  });
  // The per-panel settings popup (UI-edit-mode flyout) is app-global so every
  // scene's panels are editable.
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
    textureResolver,
    content,
    panels: null,
    logs: null,
  };
  scenes.setContext(ctx);

  // Assets live on the world server the client logs into. On login, repoint the
  // texture resolver at that server's `/textures` and pull its corpus.
  client.onLoggedIn((serverUrl) => {
    const base = assetBaseFromServerUrl(serverUrl);
    if (!base) return;
    textureResolver.setRoot(texturesRoot(base));
    void reloadContent(base).catch((err) =>
      console.warn("[content] server corpus fetch failed; staying on embed", err),
    );
  });

  // Content hot-swap: poll the server for a new corpus version and reload on a
  // change. The poll reads the client's asset base each tick (empty before login
  // → no-op), so it follows login.
  startContentPolling(() => ctx.client.assetBase());
  onContentReloaded(() => {
    ctx.content = getContent();
  });

  // ── App-global title-bar tools ──────────────────────────────────────
  const settingsMenu = new SettingsMenu(topTaskbar, uiEditMode);
  // Log Out: drop the world-server connection and return to the login screen.
  settingsMenu.onLogOut = () => {
    client.logout();
    scenes.change(new LoginScene()).catch((err) => {
      console.error("webgl: return to login failed", err);
    });
  };
  // Video settings (🖥) — reads persisted frame-cap / render-scale in its
  // constructor and applies the frame cap to the shared ticker.
  const videoPanel = new VideoPanel(app, topTaskbar, uiEditMode);
  settingsMenu.onVideo = () => videoPanel.toggle();
  // Clock-sync HUD source: one snapshot seeds from `login_ok`.
  const syncHistory = new SyncHistory();
  const debugPanel = new DebugPanel(topTaskbar, uiEditMode, syncHistory);
  ctx.debugPanel = debugPanel;
  client.onClockStats((s: ClockStats) => {
    syncHistory.update(s.synced ? toSyncStats(s) : null);
  });
  client.onCallStats((stats: CallStat[]) => debugPanel.setCallStats(stats));
  client.onSubStats((snap: SubStatsSnapshot) => debugPanel.setSubStats(snap));

  // Couple UI edit mode with the per-panel settings popup.
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

  // Drive the debug HUD every frame, scene-independent: frame-time → fps, the
  // draw-call tally (0 until W4), and the atlas occupancy (empty until W4).
  app.ticker.add((deltaMS) => {
    const lod = textureResolver.lodStats();
    debugPanel.setStats(
      deltaMS,
      drawCalls.readAndReset(),
      {
        atlases: lod.pages,
        frames: lod.frames,
      },
      syncHistory.current() ?? undefined,
      buildNowStats(),
    );
  });

  /** Snapshot the live clocks each frame for the HUD's "Now" row. `undefined`
   *  until the clock has an estimate, so the rows read "—". */
  function buildNowStats(): NowStats | undefined {
    if (!client.clockIsSynced()) return undefined;
    const diag = client.clockDiag();
    return {
      syncMs: client.syncedNowMs(),
      dateMs: Date.now(),
      serverMs: client.rawServerNowMs(),
      disciplinedOffsetMs: diag.disciplinedOffsetMs,
      slew: diag.slew,
    };
  }

  await scenes.change(new LoginScene());
  app.start();
}

/** Project the client core's `ClockStats` into the debug panel's `SyncStats`. */
function toSyncStats(s: ClockStats): SyncStats {
  return {
    offsetMs: s.serverNowMs - Date.now(),
    captures: s.captures,
    bestOffsetMs: s.bestOffsetMs,
    worstOffsetMs: s.worstOffsetMs,
    clientLagMs: s.clientDelayMs,
    rttMs: s.rttMs,
    bestRttMs: s.bestRttMs,
    rttSamples: s.rttSamples,
  };
}

main().catch((e) => console.error("webgl: boot failed", e));
