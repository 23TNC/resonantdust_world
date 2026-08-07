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
