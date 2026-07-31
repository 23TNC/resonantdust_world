# Blockers — strip the lighting + shadow system

_No open blockers._

B1 asked what replaces the stripped system, and carried "does NOT block the strip" in its own title —
which is the tell that it was never a blocker. **It was answered on 2026-07-31**: the user authored
[`docs/intent/2026-07-31-rework.md`](../../intent/2026-07-31-rework.md), and the implementing stream is
[`2026-07-31-lighting-rework`](../2026-07-31-lighting-rework/README.md).

The five inputs B1 promised the answer would be made against were delivered:

| B1 promised | delivered |
|---|---|
| the ms budget the system was spending | **0.508 ms static / 0.675 ms moving**, against an unlit floor of 0.045 — ~91 % of the frame ([P1](completed.md)) |
| a capability checklist | [rework I6](../2026-07-31-lighting-rework/issues.md) — 10 items, 3 explicitly not carried forward |
| the G-buffer contents at the seam | albedo, normal, depth, z-row — P5 |
| the surviving record bands | the primitive graph; self-positioning after F8 |
| ~83 MiB of freed GPU memory | P2 measures the real figure |
