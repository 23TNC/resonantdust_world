# Todo — docs migration (remaining components)

Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Migrate each component's scattered
top-level docs into `docs/components/<group>/<name>/{design,intent,current,plan}` — **lazily**, as
we work each. Newest-first.

> ~~Physical `git mv` breaks code-comment doc-paths; those are fixed when that code is next touched,
> not now — we're only documenting.~~ ✅ **Void (2026-07-14)** — all 15 dead code→doc paths were
> repointed in one sweep (commit `d81e1e7`). Worth knowing why it mattered: `event_word.rs` and
> `vm.rs` pointed at a `docs/spacetime-tables/event-dsl.md` that hadn't existed since the
> restructure, so the DSL code couldn't be traced to the design decision it implements — which is
> part of how divergence #1 drifted for months without anyone noticing. **Keep new doc-links live**;
> a broken pointer is how a doc quietly stops being read.

- **2026-07-14** · **server/edge** — no edge-specific top-level doc remains (worldgen is inline in
  `edge/src`); create `components/server/edge/{intent,current}` from the map's edge entry + the
  worldgen/`world-on-pipeline` relationship when edge is worked. Low priority.
- **2026-07-14** · ~~**client/core** — split the client half out of pixijs's `intent/client.md`~~
  ✅ **DONE (2026-07-14).** It turned out to need **no split**: every section (host API
  `client/core/src/api.rs`, login flow, config, the `headless` binary, build) describes the headless
  Rust client, and pixijs appears only as a future *consumer*. The whole file was misfiled →
  `git mv`'d to [`components/client/core/intent/client.md`](../../components/client/core/intent/client.md),
  inbound links fixed (`docs/intent/sync.md`, pixijs's README), and `client/core` given a README.

- **2026-07-14** · ~~`docs/repo-layout.md` — keep, merge into map, or note?~~ ✅ **DECIDED: keep,
  scoped.** It's the **physical** tree + the 2026-07-11 old→new **rename key** (still needed to read
  pre-restructure history); the map is the *conceptual* truth. Merging would bury a historical
  rename table in the map; deleting would lose the key. Both now say so and cross-link.
- **2026-07-14** · **`docs/prompts/*`** (template assets) — still deferred: they move **with** the
  `bin/` → `dev/scripts/` reorg, which is itself proposed-not-done (see the map's open list). Not
  worth moving twice.
- **2026-07-14** · **server/edge** *(unchanged, deliberately)* — see below. Lazy-create says a
  component earns its folders when it's worked; edge hasn't been. The map entry is its home until
  then.
