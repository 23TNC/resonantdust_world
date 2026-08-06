# Deviations — interactions

_None._

## D1 — the wrap drill runs in the eval's tests, not live {#d1}

_2026-08-06, P3._ The item asked for a hand-injected drink with `set_tic` across a u16 wrap
seam. `set_tic` is NOT injectable — it is stamped by the composing reducer at its own tic, by
design — so placing a row at the seam live means waiting for the master clock to reach ~65530
(hours away). The equivalent check that ran instead: the worker's ONLY tic math is
`needs_eval::satisfaction_at` + `tic_add` (verifiable by reading the arm — no ad-hoc
subtraction, the I5 rule), and the eval's unit tests pin the seam behavior on both sides
(`tic_wrap_is_handled` crosses it; `a_future_stamped_row_reads_as_fresh_not_ancient` pins the
half-window guard that live jitter exercised for real). No stale quench is reachable through
those two functions' pinned behavior.
