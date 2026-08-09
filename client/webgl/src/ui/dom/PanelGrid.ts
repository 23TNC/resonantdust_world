/**
 * The UI coordinate system: a fixed 58 × 33 cell grid over the full
 * viewport. Every panel, both taskbars, every title bar and the text
 * scale are expressed in cells; pixels are a *projection* of the grid
 * against the live viewport and are never stored.
 *
 * Design: `docs/components/client/webgl/design/panel-layout.md`
 * (work `docs/work/2026-08-09-panel-grid/`, decisions F1–F9).
 *
 * Three things to know before touching this file:
 *
 *  1. **The grid spans the FULL viewport**, not the area between the
 *     taskbars. Row 0 IS the top taskbar and row 32 IS the bottom one
 *     — that is the only way a bar can be exactly one row tall. Rows
 *     1..31 are the *field*, the rows panels may occupy.
 *
 *  2. **Edges are an integer TABLE, not a float step.** A cell rect's
 *     pixels are differences of table entries, so neighbours share one
 *     integer and the tables tile the viewport exactly. The old
 *     `origin + n * step` form is what produced the drift visible in
 *     the pre-grid corpus (`"top": "56.3295px"`).
 *
 *  3. **This is a module singleton, not an injected dependency.** It
 *     used to live on `UiEditMode`, which is optional — and one panel
 *     (`SettingsMenu`) deliberately declines it. The grid is not a
 *     mode; a panel cannot opt out of being placed. See
 *     `docs/work/2026-08-09-panel-grid/deviations.md`.
 *
 * The pure math (`gridEdges` / `projectCell` / `nearestCell` /
 * `clampCell`) is exported free of any `window` reference so it can be
 * exercised directly; the class is a thin stateful wrapper that owns
 * the live tables and the app's single resize listener.
 */

/** Columns. Fixed on every aspect — cells stretch, they do not stay
 *  square (F8). With `GRID_ROWS`, this pair is the compensation knob:
 *  when something measures wrong the first lever is the cell *count*,
 *  one line, not a special case. No derived duplicate may exist
 *  anywhere. */
export const GRID_COLS = 58;

/** Rows, INCLUDING the two taskbar rows. 58:33 = 1.758 against 16:9's
 *  1.778, so cells are square within 1.2% on a normal display. */
export const GRID_ROWS = 33;

/** First row a panel body may occupy — row 0 is the top taskbar. */
export const FIELD_ROW_FIRST = 1;

/** Last row a panel body may occupy — row 32 is the bottom taskbar. */
export const FIELD_ROW_LAST = GRID_ROWS - 2;

/** Height of the panel field in rows (31). */
export const FIELD_ROWS = FIELD_ROW_LAST - FIELD_ROW_FIRST + 1;

/** A rect in grid cells — the ONLY form panel geometry is stored in.
 *  For a panel this describes the **body**; a visible title bar is
 *  chrome occupying the row above it (F4). */
export interface CellRect {
  col:  number;
  row:  number;
  cols: number;
  rows: number;
}

/** A rect in viewport pixels — always derived, never stored. */
export interface PxRect {
  left:   number;
  top:    number;
  width:  number;
  height: number;
}

/** Build the edge table for one axis: `count + 1` integers that tile
 *  `extent` exactly. `edges[0] === 0` and `edges[count] === extent`
 *  (for an integer extent), and consecutive entries differ by at most
 *  one pixel from each other. */
export function gridEdges(extent: number, count: number): number[] {
  const edges = new Array<number>(count + 1);
  for (let i = 0; i <= count; i++) edges[i] = Math.round((i * extent) / count);
  return edges;
}

/** Clamp `i` into `[0, max]`. */
function clampIndex(i: number, max: number): number {
  return i < 0 ? 0 : i > max ? max : i;
}

/** Project a cell rect onto pixels through the two edge tables.
 *  Width / height are *differences of table entries*, never a cell
 *  size multiplied out — that is what keeps abutting panels sharing
 *  one integer boundary. */
export function projectCell(
  edgesX: readonly number[],
  edgesY: readonly number[],
  cell: CellRect,
): PxRect {
  const col0 = clampIndex(cell.col,             GRID_COLS);
  const col1 = clampIndex(cell.col + cell.cols, GRID_COLS);
  const row0 = clampIndex(cell.row,             GRID_ROWS);
  const row1 = clampIndex(cell.row + cell.rows, GRID_ROWS);
  return {
    left:   edgesX[col0],
    top:    edgesY[row0],
    width:  edgesX[col1] - edgesX[col0],
    height: edgesY[row1] - edgesY[row0],
  };
}

/** Index of the table entry nearest `px`. A linear scan: the tables
 *  are 59 / 34 entries and this runs on pointermove at worst, so
 *  clarity beats a binary search — and unlike a divide-and-estimate
 *  it stays correct when the viewport is small enough that cells are
 *  under a pixel wide. */
function nearestEdge(edges: readonly number[], px: number): number {
  let best = 0;
  let bestDist = Math.abs(edges[0] - px);
  for (let i = 1; i < edges.length; i++) {
    const dist = Math.abs(edges[i] - px);
    if (dist < bestDist) { best = i; bestDist = dist; }
  }
  return best;
}

/** Quantize a pixel rect to the nearest cell rect. Exactly inverts
 *  `projectCell` — a projected rect's edges ARE table entries, so each
 *  scan lands on its own index and `nearestCell(projectCell(r)) === r`
 *  for every cell rect. Spans floor at 1 cell so a degenerate rect
 *  can't quantize away to nothing. */
export function nearestCell(
  edgesX: readonly number[],
  edgesY: readonly number[],
  px: PxRect,
): CellRect {
  const col  = nearestEdge(edgesX, px.left);
  const row  = nearestEdge(edgesY, px.top);
  const col1 = nearestEdge(edgesX, px.left + px.width);
  const row1 = nearestEdge(edgesY, px.top  + px.height);
  return {
    col,
    row,
    cols: Math.max(1, col1 - col),
    rows: Math.max(1, row1 - row),
  };
}

/** Constraints `clampCell` enforces. `titled` costs a row: a panel
 *  showing its title bar needs the row *above* its body, which cannot
 *  be row 0 (the taskbar) — so a titled body starts at row 2. */
export interface ClampOptions {
  /** Panel currently shows a title bar (F4). Default `false`. */
  titled?:   boolean;
  /** Minimum body width in cells. Default 1. */
  minCols?:  number;
  /** Minimum body height in cells. Default 1. */
  minRows?:  number;
}

/** Force a cell rect inside the field, honouring the minimums. Size is
 *  clamped before position so a panel too large for the field is
 *  shrunk rather than pushed off it. */
export function clampCell(cell: CellRect, opts: ClampOptions = {}): CellRect {
  const titled  = opts.titled ?? false;
  const minCols = Math.max(1, opts.minCols ?? 1);
  const minRows = Math.max(1, opts.minRows ?? 1);

  // A titled panel's bar occupies the row above the body, so its body
  // cannot start before row 2; an untitled body may sit at row 1.
  const rowFirst = FIELD_ROW_FIRST + (titled ? 1 : 0);
  const rowSpan  = FIELD_ROW_LAST - rowFirst + 1;

  const cols = Math.min(Math.max(cell.cols, minCols), GRID_COLS);
  const rows = Math.min(Math.max(cell.rows, minRows), rowSpan);
  const col  = Math.min(Math.max(cell.col, 0),        GRID_COLS - cols);
  const row  = Math.min(Math.max(cell.row, rowFirst), FIELD_ROW_LAST - rows + 1);
  return { col, row, cols, rows };
}

/** Live grid over the viewport. One instance (`panelGrid` below) owns
 *  the app's single resize listener and notifies subscribers only when
 *  the tables actually change. */
class PanelGrid {
  private edgesXCache: number[] = [];
  private edgesYCache: number[] = [];
  private viewW = -1;
  private viewH = -1;
  private readonly listeners = new Set<() => void>();
  private wired = false;
  private rebuilds = 0;

  /** How many times the edge tables have actually been rebuilt. The
   *  design's promise is ONE rebuild per resize event regardless of
   *  how many panels are open — check it from the console:
   *  `panelGrid.rebuildCount` before and after a resize. */
  get rebuildCount(): number { return this.rebuilds; }

  /** Rebuild the tables against the live viewport when it has changed.
   *  Returns whether anything moved. Cheap enough to call freely — the
   *  common case is two comparisons, which is what makes it safe for
   *  every accessor below to call it and still cost one rebuild per
   *  resize even when subscribers read the tables during the
   *  broadcast. */
  private refresh(): boolean {
    const w = window.innerWidth;
    const h = window.innerHeight;
    if (w === this.viewW && h === this.viewH) return false;
    this.viewW = w;
    this.viewH = h;
    this.edgesXCache = gridEdges(w, GRID_COLS);
    this.edgesYCache = gridEdges(h, GRID_ROWS);
    this.rebuilds++;
    this.publishScale();
    return true;
  }

  /** Publish the row height as `--ui-row` on the root element.
   *
   *  This is the ONLY thing the grid writes for the UI scale, and the
   *  only place any pixel metric leaves this module. Every derived
   *  value — font sizes, paddings, gaps, button sizes — is a `calc()`
   *  off it, authored once in the `:root` block of `index.html`. So a
   *  resize re-styles every element in the app through one property
   *  write, with no traversal and no per-component recompute.
   *
   *  Fractional by design: the browser resolves `calc()` at full
   *  precision and rounds once at paint, which is more accurate than
   *  rounding here and multiplying out. */
  private publishScale(): void {
    if (typeof document === "undefined") return;
    document.documentElement.style.setProperty("--ui-row", `${this.rowHeightAt(0)}px`);
  }

  /** Attach the single resize listener. Called lazily on first use so
   *  importing this module outside a browser (a check script) doesn't
   *  touch `window`. */
  private wire(): void {
    if (this.wired || typeof window === "undefined") return;
    this.wired = true;
    window.addEventListener("resize", () => {
      if (this.refresh()) this.emit();
    });
  }

  private emit(): void {
    for (const cb of this.listeners) cb();
  }

  get edgesX(): readonly number[] { this.wire(); this.refresh(); return this.edgesXCache; }
  get edgesY(): readonly number[] { this.wire(); this.refresh(); return this.edgesYCache; }

  /** Left edge of column `i` in viewport pixels. `edgeX(58)` is the
   *  viewport width exactly. */
  edgeX(i: number): number { return this.edgesX[clampIndex(i, GRID_COLS)]; }

  /** Top edge of row `j` in viewport pixels. `edgeY(33)` is the
   *  viewport height exactly. */
  edgeY(j: number): number { return this.edgesY[clampIndex(j, GRID_ROWS)]; }

  /** Height of row `j`. Rows differ by at most a pixel; callers
   *  wanting "the" row height want `rowHeight`. */
  rowHeightAt(j: number): number { return this.edgeY(j + 1) - this.edgeY(j); }

  /** Row index whose top edge is nearest `px`. Use when a piece of
   *  chrome must be exactly as tall as the row it sits in — a title
   *  bar takes `rowHeightAt(nearestRow(outerTop))`, not `rowHeight`,
   *  because integer edge rounding lets rows differ by a pixel and
   *  that pixel is the difference between a body on its grid line and
   *  one a hair off it. */
  nearestRow(px: number): number {
    const edges = this.edgesY;
    let best = 0;
    let bestDist = Math.abs(edges[0] - px);
    for (let j = 1; j < edges.length; j++) {
      const dist = Math.abs(edges[j] - px);
      if (dist < bestDist) { best = j; bestDist = dist; }
    }
    return Math.min(best, GRID_ROWS - 1);
  }

  /** The reference row height — row 0's, which is also the top
   *  taskbar's. The unit every piece of chrome and the whole text
   *  scale derive from (F9). */
  get rowHeight(): number { return this.rowHeightAt(0); }

  /** Height of the top taskbar strip: row 0. */
  get reservedTop(): number { return this.edgeY(FIELD_ROW_FIRST); }

  /** Height of the bottom taskbar strip: row 32. */
  get reservedBottom(): number { return this.edgeY(GRID_ROWS) - this.edgeY(GRID_ROWS - 1); }

  /** Pixels for a cell rect against the live viewport. */
  project(cell: CellRect): PxRect {
    return projectCell(this.edgesX, this.edgesY, cell);
  }

  /** Nearest cell rect for a pixel rect. Exactly inverts `project`. */
  quantize(px: PxRect): CellRect {
    return nearestCell(this.edgesX, this.edgesY, px);
  }

  /** Subscribe to grid changes (viewport resize). Returns an
   *  unsubscribe fn.
   *
   *  Deliberately does NOT fire on subscribe, unlike `UiEditMode.on`:
   *  panels subscribe from their constructor and place themselves
   *  explicitly in the same path, so a synchronous callback here would
   *  re-enter a half-built panel. */
  on(cb: () => void): () => void {
    this.wire();
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }
}

/** The app's grid. A singleton by design — see the header. */
export const panelGrid = new PanelGrid();
