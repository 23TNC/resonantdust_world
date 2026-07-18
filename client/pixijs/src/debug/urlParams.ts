//! Debug URL query parameters — conveniences for driving the client without the UI,
//! read once from `location.search`. Supported:
//!
//!   ?user=<name>    auto-login as <name> on the default server (skips the login form)
//!   ?x=<tile>       initial viewport-centre tile X (default 0) — jump the camera there
//!   ?y=<tile>       initial viewport-centre tile Y (default 0)
//!   ?ambient=<f>    ambient-floor intensity (e.g. 1.5) — brighten the whole scene instead
//!                   of fighting the lighting/shadows while debugging
//!   ?grid           overlay a debug grid over the viewport (gfx debug layer): red tile
//!                   boundaries, magenta zone boundaries, blue region boundaries — so cell,
//!                   zone and region edges are easy to read. `?grid=0`/`grid=false` turns it off.
//!   ?nocursorlight  disable the cursor/hover point light — and therefore the shadow pass it
//!                   casts (the shadow pass has no light → it just blanks). Lets you read the
//!                   non-shadow draw-call count in isolation. Pair with `?ambient=<f>` to still
//!                   see the scene.
//!
//! e.g. http://localhost:5173/?user=Developer&x=9&y=3&ambient=1.5&grid

/** Parsed debug params. `null` = absent (use the normal default). */
export interface DebugParams {
  user: string | null;
  x: number | null;
  y: number | null;
  ambient: number | null;
  grid: boolean;
  noCursorLight: boolean;
}

/** Parse a query value as a finite number, or `null` if absent/malformed. */
function num(v: string | null): number | null {
  if (v === null) return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

/** A bare flag param: present is `true` unless explicitly `0`/`false`/`no` (so `?grid`,
 *  `?grid=1`, `?grid=true` all enable; `?grid=0` disables). Absent is `false`. */
function flag(p: URLSearchParams, key: string): boolean {
  if (!p.has(key)) return false;
  const v = (p.get(key) ?? "").trim().toLowerCase();
  return v !== "0" && v !== "false" && v !== "no";
}

/** Read the debug params from the current URL. Cheap; call at scene entry. */
export function debugParams(): DebugParams {
  const p = new URLSearchParams(window.location.search);
  const user = p.get("user");
  return {
    user: user && user.trim() !== "" ? user.trim() : null,
    x: num(p.get("x")),
    y: num(p.get("y")),
    ambient: num(p.get("ambient")),
    grid: flag(p, "grid"),
    noCursorLight: flag(p, "nocursorlight"),
  };
}
