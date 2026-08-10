//! The live-edit PREVIEW — a second `Viewport` showing the selected object, with zoom and pan
//! (`2026-08-09-live-edit` F1).
//!
//! **Exactly ONE of these exists, for the app's lifetime.** That is not tidiness, it is the
//! spike's constraint. A `Viewport` owns its own WebGL2 context, and the browser was measured
//! granting extra contexts happily up to a cap and then **evicting the oldest** — the world
//! viewport — with no error thrown. One extra context against a cap of 16 is comfortably safe;
//! one per panel-open blanks the game on the sixteenth `/edit`. So this is created lazily on
//! first use, reused across every open and close, and never destroyed.
//!
//! It renders the GEO tier (flat coloured boxes with the kind's glyph) rather than the textured
//! world: a `TextureResolver` is bound to *its own* GL context and cannot be shared across two,
//! so texturing the preview means every atlas page resident twice — the cost the spike
//! deliberately did not pay before someone had looked at the cheap version. Attaching a resolver
//! later is one call.

import { Viewport } from "../../viewport/Viewport";

/** The lazily-created singleton. Module-level so it survives panel destruction. */
let shared: Viewport | null = null;
let raf = 0;

/** The one preview viewport, created on first call. */
function instance(): Viewport {
  if (!shared) {
    shared = new Viewport();
    shared.canvas.style.cssText = "display:block;width:100%;height:100%;";
  }
  return shared;
}

/** Drives the preview: mounts the shared viewport into a host element, follows a world point,
 *  and wires wheel-zoom + drag-pan. Constructing several of these is safe — they all share the
 *  one `Viewport`, which is the point. */
export class PreviewViewport {
  private readonly host: HTMLElement;
  /** Where the camera looks. Follows the selection until the user pans, then holds — panning is
   *  a statement that they want to look somewhere specific. */
  private anchor: { x: number; y: number } | null = null;
  private userPanned = false;
  private dragging = false;
  private lastPointer: { x: number; y: number } | null = null;

  constructor(host: HTMLElement) {
    this.host = host;
    const vp = instance();
    host.appendChild(vp.canvas);

    // ── zoom ──
    host.addEventListener("wheel", (e) => {
      e.preventDefault();
      e.stopPropagation();
      const step = e.deltaY < 0 ? 1.25 : 0.8;
      vp.setZoom(vp.zoom * step);
    }, { passive: false });

    // ── pan ──
    host.addEventListener("pointerdown", (e) => {
      this.dragging = true;
      this.userPanned = true;
      this.lastPointer = { x: e.clientX, y: e.clientY };
      host.setPointerCapture(e.pointerId);
      e.stopPropagation();
    });
    host.addEventListener("pointermove", (e) => {
      if (!this.dragging || !this.lastPointer || !this.anchor) return;
      // Screen delta → world delta through the camera's own scale, so panning tracks the
      // cursor at any zoom rather than drifting.
      const scale = vp.camera.renderScale || 1;
      this.anchor = {
        x: this.anchor.x - (e.clientX - this.lastPointer.x) / scale,
        y: this.anchor.y - (e.clientY - this.lastPointer.y) / scale,
      };
      this.lastPointer = { x: e.clientX, y: e.clientY };
      vp.setAnchor(this.anchor.x, this.anchor.y);
      e.stopPropagation();
    });
    const endDrag = (e: PointerEvent): void => {
      this.dragging = false;
      this.lastPointer = null;
      try { host.releasePointerCapture(e.pointerId); } catch { /* already released */ }
    };
    host.addEventListener("pointerup", endDrag);
    host.addEventListener("pointercancel", endDrag);

    if (!raf) this.startLoop();
  }

  /** Point the preview at a world position. Ignored once the user has panned — they asked to
   *  look somewhere, and yanking the camera back on the next 500 ms poll would fight them. */
  follow(x: number, y: number): void {
    if (this.userPanned) return;
    this.anchor = { x, y };
    instance().setAnchor(x, y);
  }

  /** Re-centre on the followed object and resume following. */
  recenter(): void { this.userPanned = false; }

  /** Move the shared canvas into this host — called when a second panel instance takes over. */
  remount(): void {
    const vp = instance();
    if (vp.canvas.parentElement !== this.host) this.host.appendChild(vp.canvas);
  }

  private startLoop(): void {
    const step = (): void => {
      const vp = shared;
      if (vp && vp.canvas.isConnected && vp.canvas.clientWidth > 0) {
        try { vp.tick(); } catch { /* a lost context must not take the app's rAF with it */ }
      }
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
  }
}
