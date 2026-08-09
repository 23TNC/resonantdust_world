import { NOTO_EMOJI_FAMILY } from "../../assets/fonts";

/**
 * Inline style constants for `DomPanel`. Lifted out of the main
 * class file because they're pure data — no behaviour, no
 * coupling — and at ~150 lines they were dominating the file size
 * of `DomPanel.ts` without adding to its logic.
 *
 * Every style is a `Partial<CSSStyleDeclaration>` so consumers can
 * `Object.assign(el.style, …)` directly. Theming changes live here;
 * `DomPanel` imports the constants and never inlines its own.
 */

// Shared dark fill used by every chrome surface (title bar, body,
// footer). The panel container itself stays transparent so
// a panel can select `background: none` to show whatever is beneath
// it (the world) instead. Resolved per panel via `backgroundCss`.
export const CHROME_BG = "rgba(20, 22, 30, 0.96)";

/** The panel's 1px outline colour. Toggled per panel via the `outline`
 *  option, which swaps this for `transparent` — never for `none`, so the
 *  border box (and therefore the body's rect) never changes width. */
export const PANEL_OUTLINE = "#3a3a4a";

export const PANEL_CSS: Partial<CSSStyleDeclaration> = {
  position: "fixed",
  display: "flex",
  flexDirection: "column",
  color: "#ecd6aa",
  fontFamily: "ui-monospace, monospace",
  fontSize: "var(--ui-font)",
  // No CSS min-width. The minimum is a CELL count and is per-panel
  // (`minCols` / `minRows`), enforced by `clampCell`; a global CSS floor
  // could not be overridden per panel and silently won every argument
  // with the cell clamp.
  zIndex: "20",
  overflow: "hidden",
  // REQUIRED by the cell law, not a detail. `panelGrid.project()`
  // returns border-box pixels (differences of edge-table entries), and
  // placement writes them straight to `style.width` / `style.height`.
  // Under the default `content-box` the 1px border below is added on
  // top, so every panel would sit 2px larger than its cell rect and
  // every toggle that re-reads `getBoundingClientRect().height` and
  // writes it back would grow the panel by 2px, compounding.
  boxSizing: "border-box",
  // 1px outline so overlapping panels (multiple chat / debug
  // surfaces, future DOM panels) read as distinct rectangles
  // rather than blending into each other. Per-panel `outline: false`
  // makes it TRANSPARENT rather than removing it — see
  // `DomPanel.applyOutline`.
  border: `1px solid ${PANEL_OUTLINE}`,
};

export const TITLEBAR_CSS: Partial<CSSStyleDeclaration> = {
  // Positioned so the action-button container can absolute-
  // position itself against the title bar's box and vertical-
  // center within it.
  position: "relative",
  display: "flex",
  alignItems: "center",
  // The title bar is exactly ONE GRID ROW tall. Its height is not set
  // here — `DomPanel.applyTitlebarHeight` writes the height of the
  // specific row the bar occupies, because integer edge rounding lets
  // rows differ by a pixel and `--ui-row` is only row 0's. Height is
  // LAYOUT and comes from the projection; the scale vars below are
  // SCALE. Padding is horizontal only: with a pinned height, vertical
  // padding would fight `align-items: center` and push the text off
  // centre on small viewports.
  padding: "0 var(--ui-pad)",
  boxSizing: "border-box",
  // Reserve space on the right for the absolute-positioned action
  // buttons (minimize / close). Width matches `ACTIONS_CSS.width`
  // below so the title text doesn't bleed into the actions zone.
  // Sized to the *Pixi-chrome* button footprint — two full-row
  // buttons, a gap and the right pad.
  paddingRight: "calc(var(--ui-row) * 2.5)",
  background: CHROME_BG,
  borderBottom: "1px solid #3a3a4a",
  fontSize: "var(--ui-font)",
  color: "#a0a0b0",
  fontFamily: "sans-serif",
  userSelect: "none",
  cursor: "move",
  gap: "var(--ui-pad)",
  flex: "0 0 auto",
  // Explicit `auto` is required (not the default) because the
  // `clickThrough` option sets `pointer-events: none` on the panel
  // root so body clicks fall through to the world. Chrome must stay
  // live regardless, or a click-through panel could not be dragged.
  // Spec-wise descendants with implicit `auto` should still be hit
  // targets, but every chrome element states it explicitly so drag /
  // minimize / close / resize never depend on the parent's value.
  pointerEvents: "auto",
};

export const TITLE_CSS: Partial<CSSStyleDeclaration> = {
  flex: "0 0 auto",
};

export const TABS_CSS: Partial<CSSStyleDeclaration> = {
  display: "flex",
  alignItems: "center",
  justifyContent: "flex-start",
  gap: "var(--ui-gap)",
  padding: "var(--ui-pad-sm) var(--ui-pad)",
  borderBottom: "1px solid #3a3a4a",
  background: "rgba(12, 14, 20, 0.96)",
  flex: "0 0 auto",
  // See `TITLEBAR_CSS` — explicit auto so the `clickThrough`
  // option's `pointer-events: none` on the panel root doesn't take
  // chrome interactivity with it.
  pointerEvents: "auto",
};

export const TAB_BTN_CSS: Partial<CSSStyleDeclaration> = {
  background: "none",
  border: "none",
  color: "#a0a0b0",
  cursor: "pointer",
  fontFamily: NOTO_EMOJI_FAMILY,
  fontSize: "var(--ui-font-lg)",
  padding: "var(--ui-pad-sm) var(--ui-pad)",
  borderRadius: "3px",
  lineHeight: "1",
};

export const TAB_BTN_ACTIVE_CSS: Partial<CSSStyleDeclaration> = {
  background: "rgba(40, 44, 56, 0.96)",
  color: "#ecd6aa",
};

export const ACTIONS_CSS: Partial<CSSStyleDeclaration> = {
  // Absolute-positioned inside the title bar (which is
  // `position: relative` per `TITLEBAR_CSS`) and stretched
  // edge-to-edge vertically (`top: 0; bottom: 0`) so the
  // bounding box covers the full title-bar height — not just
  // the buttons' line-height. The drag-start check skips when
  // the pointer target is inside `actionsEl`; an earlier
  // version used `top: 50%; translateY(-50%)` which left ~5px
  // strips above and below the buttons where clicks fell
  // through to the title bar and started an unwanted drag.
  // Spans the full 80px reserved by `TITLEBAR_CSS.paddingRight`
  // horizontally (`right: 0; width: 80`); buttons sit at the
  // right end via flex `justify-end` + 12px right padding, so
  // the visual layout is unchanged. The 80px is sized to the
  // *Pixi-chrome* button footprint (see `TITLEBAR_CSS`
  // padding-right) — the drag-skip zone covers the buttons' full
  // visual extent so hover-near-button clicks don't start a drag.
  // Cursor is explicit `default` so the actions zone doesn't
  // inherit `move` from the title bar — the user reads
  // "non-drag" by hover alone.
  position: "absolute",
  top: "0",
  bottom: "0",
  right: "0",
  width: "calc(var(--ui-row) * 2.5)",
  paddingRight: "calc(var(--ui-row) * 0.375)",
  boxSizing: "border-box",
  display: "flex",
  alignItems: "center",
  justifyContent: "flex-end",
  gap: "var(--ui-gap)",
  cursor: "default",
  // See `TITLEBAR_CSS` — explicit auto under a click-through root.
  pointerEvents: "auto",
};

export const ACTION_BTN_CSS: Partial<CSSStyleDeclaration> = {
  background: "none",
  border: "none",
  color: "#a0a0b0",
  cursor: "pointer",
  fontSize: "var(--ui-font-xl)",
  padding: "0",
  lineHeight: "1",
  // A full row wide, so the hit-area is comfortably larger than the
  // glyph it holds. `textAlign: center` + `border-box` centre the
  // glyph in that slot.
  width: "var(--ui-row)",
  boxSizing: "border-box",
  textAlign: "center",
};

export const BODY_CSS: Partial<CSSStyleDeclaration> = {
  flex: "1 1 auto",
  overflow: "auto",
  display: "flex",
  flexDirection: "column",
  minHeight: "0",
  // Default opaque chrome backdrop. Overridden per panel by the
  // `background` option — `none` makes the body transparent so the
  // world shows through. `applyBackground` writes the resolved value
  // onto the title bar, body and footer together.
  background: CHROME_BG,
};

/** Default background applied to footers installed via
 *  `DomPanel.setFooter`. Footers used to inherit the panel's bg —
 *  now that the panel container is transparent, each footer paints
 *  its own backdrop so the chat input row (and any future footer)
 *  stays opaque over the canvas. */
export const FOOTER_BG = CHROME_BG;

/** Size (px) of the square resize-corner handle. Edge handles use
 *  this as the offset they skip past so they don't overlap the
 *  corner — see `RESIZE_EDGE_*_CSS` / `edgeXCssFor` / `edgeYCssFor`
 *  below. */
export const RESIZE_CORNER_SIZE = 14;

/** Base resize-corner CSS — size + grip lines + cursor. The
 *  `position: absolute` is shared across all four corner variants,
 *  but the actual `top` / `right` / `bottom` / `left` anchors come
 *  from `resizeCornerCssFor` below to pin the corner to the right
 *  edge of the panel. */
export const RESIZE_CORNER_CSS: Partial<CSSStyleDeclaration> = {
  position: "absolute",
  right: "0",
  bottom: "0",
  width:  `${RESIZE_CORNER_SIZE}px`,
  height: `${RESIZE_CORNER_SIZE}px`,
  cursor: "nwse-resize",
  // See `TITLEBAR_CSS` — explicit auto under a click-through root.
  pointerEvents: "auto",
  // Two small diagonal lines as a visual grip hint.
  background:
    "linear-gradient(135deg, transparent 0%, transparent 40%, " +
    "rgba(160,160,176,0.5) 41%, rgba(160,160,176,0.5) 45%, " +
    "transparent 46%, transparent 60%, " +
    "rgba(160,160,176,0.5) 61%, rgba(160,160,176,0.5) 65%, " +
    "transparent 66%)",
};

/** Hit-area thickness (px) of each edge resize handle. Thin
 *  enough that the strip doesn't eat noticeable area off the
 *  body region — the user notices it via the cursor change, not
 *  a visible band — but wide enough to actually click on. */
export const RESIZE_EDGE_THICKNESS = 4;

/** Base CSS for the width-resize edge handle (vertical strip on
 *  the left or right side of the panel, depending on anchor).
 *  Pairs with `edgeXCssFor` for per-corner positioning. */
export const RESIZE_EDGE_X_CSS: Partial<CSSStyleDeclaration> = {
  position: "absolute",
  width:    `${RESIZE_EDGE_THICKNESS}px`,
  cursor:   "ew-resize",
  pointerEvents: "auto",
};

/** Base CSS for the height-resize edge handle (horizontal strip
 *  along the top or bottom of the panel, depending on anchor).
 *  Pairs with `edgeYCssFor` for per-corner positioning. */
export const RESIZE_EDGE_Y_CSS: Partial<CSSStyleDeclaration> = {
  position: "absolute",
  height:   `${RESIZE_EDGE_THICKNESS}px`,
  cursor:   "ns-resize",
  pointerEvents: "auto",
};

/** Per-corner positioning + cursor for the resize handle. Two pairs
 *  of cursor styles: `tl` and `br` share `nwse-resize` (the
 *  diagonal `\`), while `tr` and `bl` share `nesw-resize` (`/`).
 *  Use these to repaint the corner when the panel's anchor flips —
 *  the grab corner is always opposite the anchor. */
export function resizeCornerCssFor(
  corner: "tl" | "tr" | "bl" | "br",
): Partial<CSSStyleDeclaration> {
  const isRight  = corner.endsWith("r");
  const isBottom = corner.startsWith("b");
  const cursor   = (corner === "tl" || corner === "br") ? "nwse-resize" : "nesw-resize";
  return {
    top:    isBottom ? "auto" : "0",
    bottom: isBottom ? "0"    : "auto",
    left:   isRight  ? "auto" : "0",
    right:  isRight  ? "0"    : "auto",
    cursor,
  };
}

/** Per-corner positioning for the width-resize edge handle.
 *  Sits on the same side as the resize corner (the side opposite
 *  the anchor), runs the full height of the panel minus the
 *  corner's footprint so the two handles don't overlap. */
export function edgeXCssFor(
  corner: "tl" | "tr" | "bl" | "br",
): Partial<CSSStyleDeclaration> {
  const isRight  = corner.endsWith("r");
  const isBottom = corner.startsWith("b");
  return {
    top:    isBottom ? "0" : `${RESIZE_CORNER_SIZE}px`,
    bottom: isBottom ? `${RESIZE_CORNER_SIZE}px` : "0",
    left:   isRight  ? "auto" : "0",
    right:  isRight  ? "0"    : "auto",
  };
}

/** Per-corner positioning for the height-resize edge handle.
 *  Sits along the same horizontal side as the resize corner,
 *  spanning the full width minus the corner's footprint. */
export function edgeYCssFor(
  corner: "tl" | "tr" | "bl" | "br",
): Partial<CSSStyleDeclaration> {
  const isRight  = corner.endsWith("r");
  const isBottom = corner.startsWith("b");
  return {
    top:    isBottom ? "auto" : "0",
    bottom: isBottom ? "0"    : "auto",
    left:   isRight  ? "0" : `${RESIZE_CORNER_SIZE}px`,
    right:  isRight  ? `${RESIZE_CORNER_SIZE}px` : "0",
  };
}
