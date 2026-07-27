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
