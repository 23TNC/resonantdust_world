//! URL query parameters — a way to drive the client without the UI, read once from
//! `location.search`. The query is unified with the in-game **chat commands**: every param
//! (except `user`) names a chat command that runs ONCE after login, so `?grid=1` does the
//! same thing as typing `/grid 1`. See {@link parseUrl}.
//!
//!   ?user=<name>    auto-login as <name> on the remembered server (skips the login form). NOT a
//!                   command — it's the login identity. Absent → the form waits for a manual login;
//!                   the other params still run once that manual login completes.
//!
//! Everything else is `?<command>=<args>` (or a bare `?<command>` flag), replayed after login as
//! `/<command> <args>`. Args split on spaces/commas. Current commands (see WorldScene):
//!
//!   ?grid           → /grid               overlay the tile/zone/region debug grid (`?grid=0` off)
//!   ?focus=<x>,<y>  → /focus <x> <y>      jump the camera centre to a tile (args split on the comma)
//!   ?zoom=<level>   → /zoom <level>       set the absolute zoom (1 = native, 2 = 2× in, 0.5 = out)
//!
//! e.g. http://localhost:5173/?user=Developer&focus=9,3&zoom=2&grid

/** One URL-passed command: the chat-command name + its whitespace/comma-split args. */
export interface UrlCommand {
  name: string;
  args: string[];
}

/** Parsed URL: the special `user` login identity (or null) + the ordered command queue. */
export interface UrlParams {
  user: string | null;
  commands: UrlCommand[];
}

/** Interpret a command ARG as an on/off flag — on unless it's an explicit `0`/`false`/`no`/`off`.
 *  A missing arg falls back to `dflt` (so a bare `/grid` enables, `/grid 0` disables). Shared by
 *  the flag-style commands so chat + URL parse identically. */
export function argFlag(arg: string | undefined, dflt = true): boolean {
  if (arg === undefined) return dflt;
  const v = arg.trim().toLowerCase();
  return v !== "0" && v !== "false" && v !== "no" && v !== "off";
}

/** Read `user` + the command queue from the current URL. Cheap; call at boot / scene entry.
 *  Every param except `user` maps straight to a `{name, args}` command — the value splits on
 *  spaces/commas, so `?focus=5,5` → `/focus 5 5`. */
export function parseUrl(): UrlParams {
  const p = new URLSearchParams(window.location.search);
  const rawUser = (p.get("user") ?? "").trim();
  const commands: UrlCommand[] = [];
  for (const [rawKey, rawVal] of p.entries()) {
    const name = rawKey.toLowerCase();
    if (name === "user") continue; // login identity, not a command
    const val = rawVal.trim();
    commands.push({ name, args: val ? val.split(/[\s,]+/) : [] });
  }
  return { user: rawUser !== "" ? rawUser : null, commands };
}
