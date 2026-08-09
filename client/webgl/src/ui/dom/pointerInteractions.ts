/**
 * Reusable pointer-gesture wiring for DOM elements. Two helpers, one
 * shape:
 *   - `attachDrag` — pointerdown on a handle initiates a drag that
 *     repositions a target via `left` / `top` until pointerup.
 *   - `attachResize` — pointerdown on a corner initiates a drag
 *     that resizes a target via `width` / `height` until pointerup.
 *
 * Both attach a one-shot `pointerdown` listener to the handle and,
 * during the gesture, register `pointermove` / `pointerup` listeners
 * on `document` — that way the gesture survives the cursor leaving
 * the handle's bounds, which routinely happens during fast motion.
 * Listeners self-remove on `pointerup`.
 *
 * No cleanup function is returned: the `pointerdown` listener on
 * the handle lives for the lifetime of the element. When the element
 * is destroyed, the listener goes with it. `DomPanel` doesn't ever
 * disable drag / resize on a per-instance basis after attach.
 */

/** The grid's edge tables, read by the drag / resize helpers on every
 *  pointermove. Motion lands on a table entry rather than on a
 *  multiple of a step — the two are NOT equivalent: a float step
 *  re-rounds per gesture and drifts, while an edge is one shared
 *  integer both neighbours agree on. See
 *  `docs/components/client/webgl/design/panel-layout.md`.
 *
 *  Returning `null` from the hook keeps motion pixel-perfect. */
export interface SnapEdges {
  /** Column edges, `GRID_COLS + 1` ascending integers. */
  x: readonly number[];
  /** Row edges, `GRID_ROWS + 1` ascending integers. */
  y: readonly number[];
}

/** Value of the table entry nearest `px`. Mirrors `PanelGrid`'s
 *  `nearestEdge` but returns the pixel rather than the index — the
 *  gesture helpers work in pixels and hand the final rect to the
 *  panel, which quantizes it once on `onEnd`. */
function snapToEdge(edges: readonly number[], px: number): number {
  let best = edges[0];
  let bestDist = Math.abs(edges[0] - px);
  for (let i = 1; i < edges.length; i++) {
    const dist = Math.abs(edges[i] - px);
    if (dist < bestDist) { best = edges[i]; bestDist = dist; }
  }
  return best;
}

export interface DragHooks {
  /** Predicate consulted on `pointerdown`. The `PointerEvent` is
   *  passed in so the caller can inspect `target` (e.g., skip
   *  drag when the press lands on a child action button that
   *  shares the same drag handle). When provided and it returns
   *  false the drag never starts. */
  shouldStart?: (e: PointerEvent) => boolean;
  /** Read on every `pointermove`. When non-null the `left` / `top`
   *  updates land on the nearest column / row edge. Read fresh each
   *  move so a viewport resize mid-drag picks up the new tables
   *  without re-attaching the listener. */
  snapEdges?: () => SnapEdges | null;
  /** Optional position clamp consulted on every `pointermove`
   *  after grid snap. Returns the inclusive `left` / `top`
   *  bounds the target may sit at — the helper clamps the
   *  applied position into that box before writing CSS. Use to
   *  keep the dragged element inside a safe area (viewport
   *  minus taskbar reserves, etc.). Read fresh each move so the
   *  caller can adjust as the viewport reflows. */
  clampPosition?: () => { minLeft: number; minTop: number; maxLeft: number; maxTop: number };
  /** Fires on every `pointermove` while dragging, after the
   *  `left` / `top` update has been applied. Optional. */
  onMove?: () => void;
  /** Fires once on `pointerup`, after the gesture's `pointermove`
   *  listener has been torn down. Use for persistence / snapshotting. */
  onEnd?: () => void;
}

/**
 * Make `target` draggable via pointerdown on `handle`. Drops the
 * `right` / `bottom` anchors on first move so user-applied
 * `left` / `top` wins for the rest of the session. Caller is
 * responsible for any persistence — wire through `hooks.onEnd`.
 */
export function attachDrag(
  handle: HTMLElement,
  target: HTMLElement,
  hooks: DragHooks = {},
): void {
  let offsetX = 0;
  let offsetY = 0;

  const onMove = (e: PointerEvent): void => {
    let x = e.clientX - offsetX;
    let y = e.clientY - offsetY;
    const edges = hooks.snapEdges?.() ?? null;
    if (edges) {
      x = snapToEdge(edges.x, x);
      y = snapToEdge(edges.y, y);
    }
    const clamp = hooks.clampPosition?.();
    if (clamp) {
      // `max(min, min(x, max))` — for a panel wider/taller
      // than the safe area, `max < min` and the outer `max`
      // wins, keeping the top-left corner visible (rather
      // than the bottom-right). Title bar / close button live
      // top-left, so that's the side users need to reach.
      x = Math.max(clamp.minLeft, Math.min(x, clamp.maxLeft));
      y = Math.max(clamp.minTop,  Math.min(y, clamp.maxTop));
    }
    target.style.left = `${x}px`;
    target.style.top  = `${y}px`;
    // Drop the right / bottom anchors once dragged — the user's
    // `left` / `top` wins for the remainder of the session (and
    // across reloads if the caller persists position).
    target.style.right  = "auto";
    target.style.bottom = "auto";
    hooks.onMove?.();
  };
  const onUp = (): void => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup",   onUp);
    hooks.onEnd?.();
  };
  handle.addEventListener("pointerdown", (e: PointerEvent) => {
    if (hooks.shouldStart && !hooks.shouldStart(e)) return;
    e.preventDefault();
    const rect = target.getBoundingClientRect();
    offsetX = e.clientX - rect.left;
    offsetY = e.clientY - rect.top;
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup",   onUp);
  });
}

/** Which corner of the target rectangle the user is grabbing.
 *  Determines the sign of the cursor-delta-to-size mapping —
 *  grabbing the bottom-right makes "cursor right" widen the
 *  target, while grabbing the bottom-left makes "cursor right"
 *  *narrow* the target (since the right edge is anchored). */
export type ResizeCorner = "tl" | "tr" | "bl" | "br";

export interface ResizeOptions {
  /** Minimum `width` (px). The pointermove handler clamps the
   *  applied width to this floor. */
  minWidth: number;
  /** Minimum `height` (px). Same shape as `minWidth`. */
  minHeight: number;
  /** Predicate consulted on `pointerdown`. When provided and it
   *  returns false the resize never starts — locks the target
   *  without unwiring the corner. */
  shouldStart?: () => boolean;
  /** Read on every `pointermove`. When non-null the size is chosen
   *  so the *moving* edge lands on a grid line — computed from the
   *  anchored edge, which stays put for the whole gesture. Sizes
   *  cannot be snapped on their own: a width is only on-grid
   *  relative to where the panel starts. */
  snapEdges?: () => SnapEdges | null;
  /** Which corner is being grabbed. Read on every `pointermove`
   *  so the caller can move the grab corner around at runtime
   *  (e.g., flipping it based on an anchor) without re-attaching
   *  the resize listener. Defaults to `"br"` — bottom-right
   *  grab, the classic "drag to enlarge" behaviour. */
  corner?: () => ResizeCorner;
  /** Restrict the gesture to one dimension. `"both"` (default)
   *  is the corner-grab behaviour — width and height move
   *  together. `"x"` only writes `width` on each move; `"y"`
   *  only writes `height`. Combined with `corner`, the same
   *  helper backs both the corner handle (axis `"both"`) and
   *  the two adjacent edge handles (axis `"x"` or `"y"`). Read
   *  on every `pointermove` for the same runtime-flip reasons
   *  as `corner`. */
  axis?: () => "both" | "x" | "y";
  /** Optional size clamp consulted on every `pointermove`.
   *  Returns the maximum width / height the target may grow to
   *  before the moving edge would push past a safe-area
   *  boundary. Use to keep a resized panel inside the viewport
   *  — the caller computes the cap from the anchored corner's
   *  position and the safe-area extent. The `minWidth` /
   *  `minHeight` floors still apply; clamp values below those
   *  are silently raised to the floor. */
  clampSize?: () => { maxWidth: number; maxHeight: number };
  /** Fires on every `pointermove` while resizing, after the
   *  `width` / `height` update has been applied. */
  onMove?: () => void;
  /** Fires once on `pointerup`, after the gesture's listeners
   *  have been torn down. */
  onEnd?: () => void;
}

/**
 * Make `target` resizable via pointerdown on `corner`. Updates
 * `width` / `height` based on cursor delta from the gesture start,
 * clamped to `opts.minWidth` / `opts.minHeight`. Caller persists
 * the final size via `opts.onEnd`.
 *
 * `stopPropagation` on the corner's pointerdown so the click
 * doesn't bubble to a parent drag handle (otherwise a corner-grab
 * would also initiate a drag).
 */
export function attachResize(
  corner: HTMLElement,
  target: HTMLElement,
  opts: ResizeOptions,
): void {
  let startX = 0;
  let startY = 0;
  let startW = 0;
  let startH = 0;
  // The target's edges at gesture start. Whichever corner is grabbed,
  // the OPPOSITE edges are anchored and hold still for the whole
  // gesture — they're what a snapped size is measured from.
  let startLeft   = 0;
  let startTop    = 0;
  let startRight  = 0;
  let startBottom = 0;

  const onMove = (e: PointerEvent): void => {
    // Sign of the cursor-delta-to-size mapping depends on which
    // corner is being grabbed. Right / bottom corner edges push
    // size positive; left / top corner edges push it negative
    // (the opposite edge is anchored, so growing requires the
    // grabbed edge to move *away* from the cursor).
    const corner = opts.corner?.() ?? "br";
    const sx = corner.endsWith("r") ? 1 : -1;
    const sy = corner.startsWith("b") ? 1 : -1;
    const dx = (e.clientX - startX) * sx;
    const dy = (e.clientY - startY) * sy;
    let w = Math.max(opts.minWidth,  startW + dx);
    let h = Math.max(opts.minHeight, startH + dy);
    const edges = opts.snapEdges?.() ?? null;
    if (edges) {
      // Snap the moving edge, then read the size back off the
      // anchored one. Snapping `w` directly would put the size on a
      // grid multiple while leaving the moving edge between lines.
      w = sx > 0
        ? snapToEdge(edges.x, startLeft + w) - startLeft
        : startRight - snapToEdge(edges.x, startRight - w);
      h = sy > 0
        ? snapToEdge(edges.y, startTop + h) - startTop
        : startBottom - snapToEdge(edges.y, startBottom - h);
      w = Math.max(opts.minWidth,  w);
      h = Math.max(opts.minHeight, h);
    }
    const clamp = opts.clampSize?.();
    if (clamp) {
      w = Math.max(opts.minWidth,  Math.min(w, clamp.maxWidth));
      h = Math.max(opts.minHeight, Math.min(h, clamp.maxHeight));
    }
    const axis = opts.axis?.() ?? "both";
    if (axis !== "y") target.style.width  = `${w}px`;
    if (axis !== "x") target.style.height = `${h}px`;
    opts.onMove?.();
  };
  const onUp = (): void => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup",   onUp);
    opts.onEnd?.();
  };
  corner.addEventListener("pointerdown", (e: PointerEvent) => {
    if (opts.shouldStart && !opts.shouldStart()) return;
    e.preventDefault();
    e.stopPropagation();
    const rect = target.getBoundingClientRect();
    startW = rect.width;
    startH = rect.height;
    startX = e.clientX;
    startY = e.clientY;
    startLeft   = rect.left;
    startTop    = rect.top;
    startRight  = rect.right;
    startBottom = rect.bottom;
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup",   onUp);
  });
}
