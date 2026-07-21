//! MaxRects rectangle bin-packing — the allocator behind every `TextureAtlas`.
//!
//! Masters are now square powers of two again (see `bin/art`), so a quadtree
//! allocator would pack them cleanly — but MaxRects packs squares correctly too,
//! and still absorbs the odd non-square page (previews, linked atlases), so it
//! stays until a dedicated quadtree packer lands. MaxRects: maintain the set of
//! maximal free rectangles and, for each insert, pick the free rect that wastes
//! the least edge — the Best-Short-Side-Fit (BSSF) heuristic, the strongest
//! single-bin rule from Jukka Jylänki's survey, simple enough to keep in one file.

/** A placed (or free) axis-aligned rectangle, in atlas pixel space. */
export interface PackedRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export class MaxRectsPacker {
  readonly width: number;
  readonly height: number;

  /** Maximal free rectangles. Invariant: none is fully contained by another. */
  private free: PackedRect[];
  /** Sum of inserted rect areas — drives occupancy stats. */
  private usedArea = 0;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.free = [{ x: 0, y: 0, width, height }];
  }

  get occupancy(): number {
    return this.usedArea / (this.width * this.height);
  }

  /** Reserve a `w × h` region. Returns its placement, or `null` if it won't
   *  fit in any remaining free space. */
  insert(w: number, h: number): PackedRect | null {
    const node = this.findBestFit(w, h);
    if (!node) return null;
    this.place(node);
    this.usedArea += w * h;
    return node;
  }

  /** Best-Short-Side-Fit: the free rect leaving the smallest leftover short
   *  side (ties broken by the smaller long side). */
  private findBestFit(w: number, h: number): PackedRect | null {
    let best: PackedRect | null = null;
    let bestShort = Infinity;
    let bestLong = Infinity;
    for (const fr of this.free) {
      if (w > fr.width || h > fr.height) continue;
      const leftoverH = fr.width - w;
      const leftoverV = fr.height - h;
      const shortSide = Math.min(leftoverH, leftoverV);
      const longSide = Math.max(leftoverH, leftoverV);
      if (shortSide < bestShort || (shortSide === bestShort && longSide < bestLong)) {
        best = { x: fr.x, y: fr.y, width: w, height: h };
        bestShort = shortSide;
        bestLong = longSide;
      }
    }
    return best;
  }

  /** Carve `node` out of every overlapping free rect, then drop any free rect
   *  now contained by another (the MaxRects prune step). */
  private place(node: PackedRect): void {
    const next: PackedRect[] = [];
    for (const fr of this.free) {
      if (!this.splitInto(fr, node, next)) next.push(fr);
    }
    this.free = next;
    this.prune();
  }

  /** If `node` overlaps `fr`, push `fr`'s up-to-four non-overlapping remainders
   *  onto `out` and return true. Returns false (no remainders pushed) when they
   *  don't intersect. */
  private splitInto(fr: PackedRect, node: PackedRect, out: PackedRect[]): boolean {
    if (
      node.x >= fr.x + fr.width ||
      node.x + node.width <= fr.x ||
      node.y >= fr.y + fr.height ||
      node.y + node.height <= fr.y
    ) {
      return false;
    }
    // Slice off the parts of `fr` above / below / left / right of `node`.
    if (node.y > fr.y) {
      out.push({ x: fr.x, y: fr.y, width: fr.width, height: node.y - fr.y });
    }
    if (node.y + node.height < fr.y + fr.height) {
      out.push({
        x: fr.x,
        y: node.y + node.height,
        width: fr.width,
        height: fr.y + fr.height - (node.y + node.height),
      });
    }
    if (node.x > fr.x) {
      out.push({ x: fr.x, y: fr.y, width: node.x - fr.x, height: fr.height });
    }
    if (node.x + node.width < fr.x + fr.width) {
      out.push({
        x: node.x + node.width,
        y: fr.y,
        width: fr.x + fr.width - (node.x + node.width),
        height: fr.height,
      });
    }
    return true;
  }

  /** Drop free rects fully contained by another — keeps the set "maximal" so
   *  the list stays small and `findBestFit` doesn't double-count space. */
  private prune(): void {
    for (let i = 0; i < this.free.length; i++) {
      for (let j = i + 1; j < this.free.length; j++) {
        if (contains(this.free[j], this.free[i])) {
          this.free.splice(i, 1);
          i--;
          break;
        }
        if (contains(this.free[i], this.free[j])) {
          this.free.splice(j, 1);
          j--;
        }
      }
    }
  }
}

/** True if `outer` fully encloses `inner`. */
function contains(outer: PackedRect, inner: PackedRect): boolean {
  return (
    inner.x >= outer.x &&
    inner.y >= outer.y &&
    inner.x + inner.width <= outer.x + outer.width &&
    inner.y + inner.height <= outer.y + outer.height
  );
}
