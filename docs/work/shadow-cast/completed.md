# Completed — shadow-cast

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## S0–S5 · DONE + verified in-browser 2026-07-20

`?focus=100,50&shadowcast`: **5 cold lights** in the (100,50) zone cast **billboard shadows** from the
in-radius trees (the cold cache's `standingPrims`), each light's shadow written as **one bit** of the RED
byte of a ping-pong bitfield RT (`shadow-a`/`shadow-b`), displayed as **5 distinct colours** (red / green /
blue / yellow / magenta), overlaps blending additively (pink = red+blue, cyan = green+blue, white =
several). The world shows through where unshadowed. **Both things this experiment set out to prove hold:**

- **(a) Cast shadows using 5 bits in one channel** — 5 lights' shadows coexist as 5 decodable bits in the
  RED byte; the projected billboard quads emanate from the trees, away from each light.
- **(b) Incremental updates** — one light moves per second (round-robin) and only its colour relocates;
  the other four colours **persist pixel-stable** across the move. That persistence is the proof of
  **carry-forward**: the combine copies the non-dirty bits through and re-casts only the dirty light. If
  carry-forward were broken they'd flicker/vanish each frame — they don't.

**The mechanism (a miniature of `shadow-hot` → `shadow-cold`):** per update — cast the dirty light `k`'s
in-radius shadows into a `mask` RT (screen-space billboard quads, `Graphics`); a **combine** pass reads
`src` bitfield + `mask` → writes `dest` (clear bit `k`, set where the mask covers, carry the other 4 bits);
swap `src`↔`dest`; display `dest`. Idle frames (no move) just display the current buffer — no ping-pong.
Source ≠ destination throughout (no feedback loop). All unorm RGBA8, **A=1, float-mod** (ES 1.00), **alpha
never used for data**.

Implementation: [`shadowCastShaders.ts`](../../../client/pixijs/src/game/viewport/shadowCastShaders.ts)
(combine + 5-colour display) + [`shadowCast.ts`](../../../client/pixijs/src/game/viewport/shadowCast.ts)
(5 lights, 2 ping-pong RTs + mask, cast/move/queue) + `SquareCache.standingPrims` re-added; toggled by
`/shadowcast`.

**Conclusion:** casting real shadows into a ping-pong bitfield with cheap per-light incremental updates
works. This is the last piece before the real [`shadows`](../shadows/README.md) engine — the `shadow-hot`
→ `shadow-cold` seam is now proven end to end.

