# Forks — spacetime rewrite

Decision points with more than one viable path + which we chose + why. Chronological.

- **2026-07-14** · **Next tranche after the behavioral core** — with Section A (behavioral
  mechanisms) all closed, the options were: **(A)** browser integration, **(B)** the
  representation re-keys, **(C)** stop & review. **Chose A.** **Why:** A was the only *unblocked*
  path (B needs the in-flux object-model decisions — see [blockers.md](blockers.md) B-1, and
  deciding them unilaterally is the failure that started this pass); A makes the proven pipeline
  real + visible and surfaces the next real problems (it caught the `home_shard` mint_server=0
  bug). · detail: `docs/issues/006-next-tranche-fork.md`
- **2026-07-14** · **Worker/master standup shape** — **(B-full)** mirror edge's compose +
  up/down/logs/deploy wiring; **(B-lite)** a thin `rd run` over a persistent container;
  **(B-minimal)** doc-only recipe. **Chose B-lite.** **Why:** least surface, directly matches the
  proven throwaway recipe; edge's heavier pattern is overkill for two debug binaries. ·
  → `rd run` in [completed.md](completed.md) #6.
- **2026-07-14** · **How npc pauses** — **(a)** relay the `paused` flag to clients (npc gates on
  `Event::Paused`); **(b)** npc infers pause from a frozen tic in state traffic. **Chose (a).**
  **Why:** authoritative + direct; the shard's `tic_meta` is the natural broadcast medium (edge
  relays it per-subscriber), and (b) is fragile (can't tell paused from idle). · → `/pause` in
  [completed.md](completed.md).
