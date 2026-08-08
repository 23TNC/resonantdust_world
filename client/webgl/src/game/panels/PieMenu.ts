//! The context PIE MENU (input-rework F1/F7, `docs/components/client/webgl/design/input-model.md`):
//! one `menu_text` rounded rect per available interaction, at a FIXED radius from the click
//! point, equal angular spacing from 12 o'clock, anchored in screen space. A DOM overlay (the
//! ConditionCards pattern — position-fixed siblings of the panels), pointer events ON. ONE
//! menu at a time; ANY other input dismisses it (document-capture listeners here, so panel
//! clicks and keys dismiss without the scene knowing); a rect click reports the pick, then
//! closes. Spacing refinements are explicitly deferred (user: "We will work on the spacing
//! later").

import { Z_CHROME_BASE } from "../../ui/dom/DomPanel";

/** One offered interaction — the wasm `tileMenuOptions` row (already predicate- and
 *  location-filtered; the menu never re-decides availability). */
export interface PieMenuOption {
  name: string;
  menuText: string;
  /** The interaction's u32 gameplay `definition_reference`. */
  reference: number;
  /** The carrier binding's magnitude (0 when unauthored). */
  magnitude: number;
  /** The input SIGNATURE — the composer binds these by the reserved vocabulary (F5). */
  inputs: string[];
}

/** Fixed rect-center distance from the click point (user: fixed distance, equal angles). */
const RADIUS_PX = 72;

export class PieMenu {
  private root: HTMLDivElement | null = null;

  get isOpen(): boolean {
    return this.root !== null;
  }

  /** Open at client `(cx, cy)` with `options` (empty = nothing opens — F7); `onPick` fires
   *  AFTER the menu closes. Replaces any open menu (one at a time). */
  open(cx: number, cy: number, options: PieMenuOption[], onPick: (o: PieMenuOption) => void): void {
    this.close();
    if (options.length === 0) return;
    // The BODY, not #app: a transformed ancestor turns position:fixed into
    // ancestor-relative and the rects drift off the cursor (seen live).
    const host = document.body;
    const root = document.createElement("div");
    for (let i = 0; i < options.length; i++) {
      const o = options[i];
      // 12 o'clock first, clockwise, equal spacing.
      const angle = -Math.PI / 2 + (i * 2 * Math.PI) / options.length;
      const x = cx + Math.cos(angle) * RADIUS_PX;
      const y = cy + Math.sin(angle) * RADIUS_PX;
      const b = document.createElement("div");
      b.textContent = o.menuText;
      // bug-sweep F1: the pie menu is CHROME — above every panel tier (the old 60000
      // sank under the game view's 320001 the moment the numeric tiers landed).
      b.style.cssText =
        `position:fixed;left:${x}px;top:${y}px;transform:translate(-50%,-50%);` +
        "padding:6px 14px;border-radius:10px;background:#1c2128;border:1px solid #444c56;" +
        "color:#adbac7;font:12px/1.4 ui-monospace,monospace;white-space:nowrap;" +
        `cursor:pointer;user-select:none;box-shadow:0 2px 8px rgba(0,0,0,0.5);z-index:${Z_CHROME_BASE + 10};`;
      b.addEventListener("pointerdown", (ev) => {
        ev.stopPropagation();
        ev.preventDefault();
        this.close();
        onPick(o);
      });
      root.appendChild(b);
    }
    host.appendChild(root);
    this.root = root;
    // ANY other input dismisses (F1): document-CAPTURE listeners run before the rects'
    // own handlers, but a rect click is inside `root` and survives the contains() check.
    document.addEventListener("pointerdown", this.onAnyPointer, true);
    document.addEventListener("wheel", this.onAnyInput, true);
    document.addEventListener("keydown", this.onAnyInput, true);
  }

  close(): void {
    if (!this.root) return;
    this.root.remove();
    this.root = null;
    document.removeEventListener("pointerdown", this.onAnyPointer, true);
    document.removeEventListener("wheel", this.onAnyInput, true);
    document.removeEventListener("keydown", this.onAnyInput, true);
  }

  dispose(): void {
    this.close();
  }

  private readonly onAnyPointer = (e: Event): void => {
    if (this.root && e.target instanceof Node && this.root.contains(e.target)) return;
    this.close();
  };

  private readonly onAnyInput = (): void => this.close();
}
