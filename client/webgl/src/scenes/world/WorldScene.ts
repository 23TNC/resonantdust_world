//! WorldScene — STUB for the W3 login-boot milestone. The real world scene hosts
//! the viewport (SquareCache G-buffer + shadows), the chat panel, movers, and the
//! world bridge — all of which ride the Pixi render layer being ported in
//! [webgl-engine W4](../../../../docs/work/webgl-engine/todo.md). Until then, a
//! successful login lands here and shows a placeholder so the login → world
//! handoff (and the whole non-render half of the client) is verifiable end-to-end.
//! Replace this file wholesale in W4 with the ported viewport-hosting scene.

import { Scene } from "../Scene";
import type { GameContext } from "../../GameContext";

export class WorldScene extends Scene {
  private el: HTMLDivElement | null = null;

  onEnter(ctx: GameContext): void {
    const el = document.createElement("div");
    el.style.cssText =
      "position:fixed;inset:0;display:flex;align-items:center;justify-content:center;" +
      "flex-direction:column;gap:8px;color:#9fb3c8;font:14px/1.5 system-ui,sans-serif;" +
      "pointer-events:none;text-align:center;";
    el.innerHTML =
      '<div style="font-size:18px;color:#d7e3ef">Logged in.</div>' +
      "<div>World viewport lands in webgl-engine W4 (port of SquareCache + shadows).</div>";
    ctx.app.mount.appendChild(el);
    this.el = el;
  }

  onExit(): void {
    this.el?.remove();
    this.el = null;
  }
}
