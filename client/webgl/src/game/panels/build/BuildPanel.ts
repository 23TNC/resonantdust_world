//! The build panel (build-walls P2, D3/D6) — a DomPanel with one sub-tab per BUILD CATEGORY,
//! populated ENTIRELY from content (`buildMenuEntries` — a kind is an icon because its
//! `:data` hook tags `tile.build`; adding a brick wall in content lights a second icon with
//! no client change). Icons draw the kind's LINKED atlas cell (0,1) — per the user — as a
//! CSS crop of the hash-addressed master URL (D6: the same art the world uses, no icon
//! assets). Clicking an icon enters WALL PLACEMENT MODE via the scene's callback; the active
//! icon highlights; right-click in the world exits (the scene clears us via
//! {@link setActive}).

import { DomPanel } from "../../../ui/dom/DomPanel";
import { panelTitle } from "../panelStrings";
import type { GameContext } from "../../../GameContext";
import type { TextureResolver } from "../../../textures";
import { lodUrl } from "../../../textures/lod";
import type { BuildEntry } from "../../world/buildMenu";

/** Category glyphs for the tab strip (fallback: the category's first letter). */
const CATEGORY_ICONS: Record<string, string> = { wall: "▦" };

export class BuildPanel extends DomPanel {
  private readonly iconEls = new Map<number, HTMLButtonElement>();
  private activeDef: number | null = null;

  constructor(
    ctx: GameContext,
    private readonly resolver: TextureResolver,
    private readonly entries: () => Map<string, BuildEntry[]>,
    private readonly onPick: (entry: BuildEntry) => void,
  ) {
    super({
      title: panelTitle("buildPanel"),
      storageKey: "build",
      minWidth: 180,
      minHeight: 120,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    this.rebuild();
  }

  /** (Re)populate the category tabs from content — called at construction and on hot-swap. */
  rebuild(): void {
    this.iconEls.clear();
    for (const [category, list] of this.entries()) {
      const body = document.createElement("div");
      body.style.cssText = "display:flex;flex-wrap:wrap;gap:6px;padding:8px;align-content:flex-start;";
      for (const entry of list) {
        const btn = document.createElement("button");
        btn.title = entry.name;
        btn.style.cssText =
          "width:48px;height:48px;padding:0;border:2px solid #555;border-radius:4px;cursor:pointer;" +
          "background-color:#222;background-repeat:no-repeat;image-rendering:pixelated;";
        const stem = `${entry.stem}/l`;
        btn.textContent = entry.name.slice(0, 2);
        // The manifest arrives async after login — apply the icon art when its hash lands
        // (bounded retry). The 4×4 linked atlas, CSS-cropped to cell (0,1) — the user's
        // chosen icon cell: background 400%×400%; position (0%, y/(rows−1) = 33.333%).
        let tries = 30;
        const applyBg = (): void => {
          const hash = this.resolver.manifestHashFor(stem);
          if (!hash) {
            if (tries-- > 0) window.setTimeout(applyBg, 1000);
            return;
          }
          btn.textContent = "";
          // one-resolution F2: the icon fetches the stem's ONE served size from the manifest.
          const size = this.resolver.maxSizeFor(stem) ?? 512;
          btn.style.backgroundImage = `url(${lodUrl(this.resolver.rootUrl(), stem, hash, size)})`;
          btn.style.backgroundSize = "400% 400%";
          btn.style.backgroundPosition = "0% 33.3333%";
        };
        applyBg();
        btn.addEventListener("click", () => {
          this.setActive(entry.defId);
          this.onPick(entry);
        });
        this.iconEls.set(entry.defId, btn);
        body.appendChild(btn);
      }
      this.addTab(category, CATEGORY_ICONS[category] ?? category.slice(0, 1).toUpperCase(), body);
    }
  }

  /** Highlight the active kind (null = placement mode exited). */
  setActive(defId: number | null): void {
    if (this.activeDef !== null) {
      const prev = this.iconEls.get(this.activeDef);
      if (prev) prev.style.borderColor = "#555";
    }
    this.activeDef = defId;
    if (defId !== null) {
      const btn = this.iconEls.get(defId);
      if (btn) btn.style.borderColor = "#ffd54a";
    }
  }
}
