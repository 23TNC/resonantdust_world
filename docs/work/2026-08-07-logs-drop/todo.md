# Plan — logs-drop

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [x] VARIABLES.md: `yields` on the CARRIER BINDING (F1) beside `magnitude`, the `logs`
      thing block, and the destroy-becomes-REPLACE note (F2) — the lumberjack RESERVED
      `yields` note consumed. Acceptance: docs-check green. → the binding schema gains
      the per-carrier `yields` lane (tree binds, shrub/cactus clear, name must resolve
      at load); the cut_down block's RESERVED note replaced by the REPLACE semantics;
      green. (The logs thing itself is corpus — P1.)

## P1 — the corpus

- [x] The `logs` thing (F3: placeholder tint, no interactions); the tree's binding
      gains `yields = "logs"`; loader validates `yields` against thing kinds, ONE
      accessor shape swept (I1). Acceptance: content-check clean; a ghost yield
      refuses loudly. → `logs` APPENDED last (a mid-file insert would collide the
      registry — noted in the corpus); `InteractionBind { name, magnitude, yields }`
      replaces the tuple through loader, both accessors, sim-version hashes (the yield
      BUMPS — sim-visible), wasm filter, and the worker's probes; ghost-yield refusal
      unit-tested; content-check clean, all gates (content tests 40, shared, core,
      worker, tsc) green.
- [x] Golden re-blessed (I4) and ALL FOUR sim crates rebuilt + restarted (I2 —
      lumberjack I11's law). Acceptance: golden diff = the authored rows; the registry
      shows the logs def after the master reseeds. → golden diff audited (the logs
      def, the struct Debug shape, tree `yields: Some("logs")` vs shrub/cactus None);
      the registry appended `logs` at id 0x200000C0 (kind 12; 198→199 defs, nothing
      renumbered); I2 BIT ANYWAY — the npc's `usable_drink` was a missed sweep site
      (its RUNTIME corpus fetch refused `yields` until rebuilt), caught by its own
      loud corpus-load error and fixed.

## P2 — the drop

- [x] The worker's destroy composer (F2): a felled carrier whose binding authors
      `yields` emits the SET with the yielded `kind_reference` instead of 0; no yield =
      today's clear. Acceptance: worker check green; the log line names the yield.
      → built + checked; the executed line carries `yielded=Some("logs")|None`; a
      registry-drift yield falls back to clearing with a WARN.
- [x] Drill: fell a TREE → logs at its cell (overlay carries the logs kind;
      `thingDefAt` reads logs — I3), the placeholder renders, a RELOAD keeps it; a
      felled SHRUB still clears. Acceptance: sql + captures + both outcomes in one log.
      → trees ×3 felled with `yielded=Some("logs")` at their fire tics; overlay rows
      carry kind 192 (logs, kind 12); `thingDefAt` reads 12; the log-brown placeholder
      renders (captures) and SURVIVES reloads. The drill un-buried TWO real bugs, both
      fixed: the browser CACHED a stale wasm silently (304s — the whole cold family
      "vanished"; cache-bust + rebuild exposed it), and the u16 TIC-RING WRAP past
      32768 made EPOCH-ZERO worldgen baselines outrank every override (felled things
      resurrected) — baselines at tic 0 now always lose (WorldBridge, both comparison
      sites; PACK re-stamps are the recorded successor). HONEST GAP: the shrub-clears
      negative was NOT log-verified — repeated clicks resolved to trees/flora (the
      user confirmed shrub menus offer Cut Down live; golden proves the shrub binding
      authors `yields: None`, so the clearing branch is loader-proven, not
      drill-proven). The user redirected mid-drill; recorded and moved on.

## P3 — the verdict

- [ ] Docs + memory truth pass: the lumberjack memory's "yields reserved, not built"
      claims amended; the index row records delivery; the hauling successor named
      (I5). Acceptance: docs-check green.
- [ ] Cold boot: the stack bounces, the logs survive as overlay state and the arcs
      (fell-with-yield, drink, wolf trips) run green; **the user's eyes close the
      stream**. Acceptance: captures + logs in completed.md.
