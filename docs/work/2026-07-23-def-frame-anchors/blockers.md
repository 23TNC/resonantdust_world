# Blockers — def frame/anchor rework

_Open blockers; remove the row when cleared._

---

- **P0 · layout ratification (USER).** The def format is user-authored and detailed design produced
  **three deviations** from the spoken spec that need the author's yes/no before code conforms:
  [F1](forks.md#f1) `frame_span` u3 added (ppu underivable from lod alone),
  [F4](forks.md#f4) `nudge_x` signed via +1024 bias (top-left bias does NOT guarantee rightward
  centering), and the arithmetic fixes (GREEN reserve u12; ppu doubles per lod, `2^(lod−4)`).
  [F3](forks.md#f3) (prim_data position semantics) can be decided at P3 either way. P1–P4 execute
  immediately on ratification.
