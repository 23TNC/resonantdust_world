# Completed — lumberjack

## 2026-08-07 — P0: the paper

- **ACTIONS.md § "The intent queue, and interactions that cost tics"**: the law written
  after §Movement — ephemeral per-pawn list (cap 5) with the in-flight queued event as
  the durable half, ONE composer (the worker's EXECUTE_INTERACTION arm), fresh order
  replaces whole, per-kind advancement (in-pass / chain-final-hop-by-trip-serial /
  queue_at completion at +N), every completion RE-VALIDATES → logged no-op when stale.
  Verified: docs-check green; the section reads without this folder.
- **VARIABLES.md TOML schema**: the three location rules enumerated (`on` / `adjacent` =
  Chebyshev ≤ 1 inclusive / `target`) with drink shown `adjacent`; `duration`'s
  completion-re-validates comment; a full `cut_down` block with `destroy = "carrier"`
  and the RESERVED `yields` successor note. Verified: docs-check green.

## 2026-08-07 — P1: the corpus

- **The loader learned the lumberjack surface**: `InteractionParams.destroy`
  (`Option<String>`, `"carrier"` the only legal value — refusal tested), `"adjacent"`
  joined the built location rules, negative durations refuse, and destroy counts as an
  effect for the at-least-one rule. The old test fixture that used `adjacent` as the
  unbuilt-location refusal now uses `orbit`; a new round-trip asserts
  `destroy/location/duration` land in params. Verified: 39 unit + 2 golden tests green.
- **The corpus authored**: `logging` stat + `lumberjack` trait (add [1]) +
  `can_fell_trees` (logging > 0) + `cut_down` (menu "Cut Down", inputs pawn/destination,
  `destroy = "carrier"`, `location = "adjacent"`, `duration = 30`); `drink` moved to
  `adjacent` and its consumed I9 RESERVED note deleted; tree/shrub/cactus bind
  `cut_down`; both human kinds author lumberjack level 1. Verified: content-check clean;
  golden diff audited row-by-row (new refs 0x8003/4/5/6_0030, drink adjacent, three
  fellable things, humans +lumberjack@1) and re-blessed.
