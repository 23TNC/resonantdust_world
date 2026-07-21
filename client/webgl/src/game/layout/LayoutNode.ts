//! LayoutNode — STUB. In the pixijs client this was a Pixi `Container` subclass
//! backing canvas-drawn panel chrome + cards. Per
//! [webgl-engine F6](../../../../docs/work/webgl-engine/forks.md) the panel chrome
//! becomes CSS and cards defer to a later sub-phase, so the canvas node type
//! collapses. `PanelManager` still references it as a **type** for its
//! node→panel tracking (a world-scene / cards concern, unused at login), so this
//! keeps a minimal structural placeholder — a parent-linked node — until the card
//! system returns. Replace/remove when cards land.

export interface LayoutNode {
  /** Parent in the node tree, or null at the root. Enough for
   *  `PanelManager.findPanelByDescendant`'s upward walk. */
  readonly parent: LayoutNode | null;
}
