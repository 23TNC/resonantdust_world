//! Debug URL query parameters — conveniences for driving the client without the UI,
//! read once from `location.search`. Supported:
//!
//!   ?user=<name>    auto-login as <name> on the default server (skips the login form)
//!   ?x=<tile>       initial viewport-centre tile X (default 0) — jump the camera there
//!   ?y=<tile>       initial viewport-centre tile Y (default 0)
//!   ?ambient=<f>    ambient-floor intensity (e.g. 1.5) — brighten the whole scene instead
//!                   of fighting the lighting/shadows while debugging
//!
//! e.g. http://localhost:5173/?user=Developer&x=9&y=3&ambient=1.5

/** Parsed debug params. `null` = absent (use the normal default). */
export interface DebugParams {
  user: string | null;
  x: number | null;
  y: number | null;
  ambient: number | null;
}

/** Parse a query value as a finite number, or `null` if absent/malformed. */
function num(v: string | null): number | null {
  if (v === null) return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
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
  };
}
