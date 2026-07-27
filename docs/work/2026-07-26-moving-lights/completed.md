# Completed — verification log

_Dated entries: what landed and **how it was checked**. Append-only; authoritative for what's done and why we
believe it. Items live in [`todo.md`](todo.md) with their boxes ticked — this file records the evidence._

_Standing rule for this stream ([I3](issues.md#i3)): an entry here names the **output** that was read — a
`debugReadShadow(cls)` hash / byte population, or a frame time — not a JS field, an input count, or a
screenshot impression. Three verifications this session were false because they stopped at the near end of the
pipeline._

## Baseline carried in from the prior session (2026-07-26, pre-P0)

Recorded here so P0's instrumented numbers have something to sit next to. Measured at **zoom 0.25, reach 16
tiles**, 123 lights:

| condition | ms/frame | fps | dirty tiles |
|---|---|---|---|
| static | 0.72 | 120 | 696 |
| 120 of 123 moving | 72.44 | 15.2 | 5 384 |

Caveats that P0 must resolve rather than inherit:
- The **dirty counter reads a constant 696 when static and appears latched** — it may not be a per-frame
  figure at all. Confirm what it reports before any conclusion rests on it.
- Not comparable to the earlier 6.03 ms / 128-light figure: that was **reach 6**, ~7× less area per light.
- The "moving" row predates [I3](issues.md#i3) — those lights moved via the debug array that
  [primitive-graph](../2026-07-25-primitive-graph/README.md) has since deleted, so the number stands as an
  order-of-magnitude signal, not a measurement to diff against.
