# Forks — decision points, options, choices

## F1 — where the self-heal helper lives (2026-07-27)

Options: (a) copy the gateway's `Conn` pattern into each of the three binaries; (b) a hand-written
module inside `server/st-bindings`; (c) a new `server/uplink` crate.

**Chose (c).** One implementation for three consumers (the gateway can migrate later); keeps the
generated-bindings crate pure (a hand-written module there risks being clobbered or entangled on
regeneration); a `path = "../uplink"` dep matches the existing convention (`../st-bindings`).
(a) is 3× drift by construction.

## F2 — startup policy: crash-fast vs retry-in-process (2026-07-27)

Options: (a) keep the panics and rely on a supervisor to restart the process; (b) the gateway
model — best-effort warm-up, rebuild-on-next-use, so startup and mid-run recovery are the same
code path.

**Chose (b).** Nothing supervises the sim binaries today (`bin/sim` just runs them), and a
partial outage (one shard down) should not kill connections that are healthy — the master should
keep ticking the shards it can reach. Crash-fast also loses the master's warm dedup state for no
benefit.

## F3 — how `Uplink` stays generic over distinct `DbConnection` types (2026-07-27)

Options: (a) a trait abstracting the SDK's per-module connection/builder types; (b) closure-based
— the caller supplies a build closure (returns `(Arc<C>, Arc<AtomicBool>)`) and an optional
async subscribe closure; `Uplink<C>` is otherwise type-agnostic.

**Chose (b).** Each module's `DbConnection` is a distinct generated type with no shared trait
covering builders + subscription builders; closures avoid trait gymnastics entirely and make the
helper unit-testable with a fake `C` (e.g. `C = ()`), which P1's acceptance relies on.


## F4 — bounce consumers after a republish, or let them self-heal (2026-07-28)

The P5 plan items said "bounce the running sim consumers" after a module/edge publish. By the
time P5 ran, P2-P4 had made every consumer SELF-HEAL (uplinks server-side, engine reconnect
client-side) — and the P3 republish drill proved the next compose lands with zero manual
steps. **Chose self-heal + a status line** over bouncing: a bounce would discard warm state
(the master's fan dedup, the npc's adoption) to solve a problem that no longer exists. The
redeploy now prints who will heal ("live sim processes self-heal in place: …") and, for the
edge, reminds that BROWSER tabs still need a reload.
