/**
 * App-wide "UI edit mode" flag. While enabled, every `DomPanel`
 * surfaces an extra row of action buttons (grid-snap, lock, hide
 * title bar) and forces its title bar visible regardless of the
 * panel's own preference. Toggled from the settings menu.
 *
 * State isn't persisted across sessions on purpose — edit mode is
 * a setup phase, not a runtime mode. Each session starts with it
 * off; the user re-enters when they want to rearrange panels.
 *
 * Listener pattern mirrors the rest of the panel system: every
 * subscriber gets a `(enabled) => void` callback on each change,
 * plus a synchronous one-shot fire on subscribe so callers don't
 * have to read `enabled` separately to set initial state.
 *
 * This class used to own the grid geometry too, with a `gridSize`
 * target cell side and taskbar-derived reserves. That is gone: the
 * grid is the app's coordinate system and lives in `PanelGrid.ts` as
 * a singleton, because `UiEditMode` is an OPTIONAL dependency that
 * one panel deliberately declines — and no panel may opt out of being
 * placed. See `docs/components/client/webgl/design/panel-layout.md`.
 */

/** Forward-declared shape so `UiEditMode` can hold a reference
 *  without importing the concrete class (avoids a circular import
 *  chain DomPanel → UiEditMode → PanelSettingsPopup → DomPanel).
 *  The real `PanelSettingsPopup` is installed from `main.ts`
 *  after construction; until then `openSettings` is a no-op. */
export interface PanelSettingsPopupLike {
  show(panel: import("./DomPanel").DomPanel): void;
  close(): void;
}

export class UiEditMode {
  private _enabled = false;
  private readonly listeners = new Set<(enabled: boolean) => void>();
  /** Shared per-panel settings popup. Set from `main.ts` after
   *  construction (see the forward-declared `PanelSettingsPopupLike`
   *  type above for the reason). `null` until then — `openSettings`
   *  silently no-ops in that window. */
  settingsPopup: PanelSettingsPopupLike | null = null;

  get enabled(): boolean { return this._enabled; }

  /** Open the shared settings popup bound to `panel`. Called by
   *  each `DomPanel`'s ⛯ button. No-op if the popup hasn't been
   *  attached yet. */
  openSettings(panel: import("./DomPanel").DomPanel): void {
    this.settingsPopup?.show(panel);
  }

  toggle(): void { this.setEnabled(!this._enabled); }

  setEnabled(enabled: boolean): void {
    if (this._enabled === enabled) return;
    this._enabled = enabled;
    for (const cb of this.listeners) cb(enabled);
  }

  /** Subscribe to changes. Listener fires immediately with the
   *  current state so subscribers can do their initial render in
   *  the same code path as updates. Returns an unsubscribe fn. */
  on(cb: (enabled: boolean) => void): () => void {
    this.listeners.add(cb);
    cb(this._enabled);
    return () => this.listeners.delete(cb);
  }
}
