# Issues — shadow-cast

_Anticipated gotchas (seeded on open 2026-07-20) — the guards each step must respect. Move to "resolved"
as they land._

---

## I-1 · Feedback loop — the combine can't read the buffer it writes — open

Setting bit `k` in the bitfield is a read-modify-write; reading and writing the **same** RT in one draw is
a framebuffer feedback loop (UB in WebGL2). **Guard:** ping-pong — the combine reads **`src`** and writes
**`dest`** (source ≠ destination — [shadows I-8](../shadows/issues.md#i-8)), and casts through a **separate
`mask`** RT ([F3](forks.md#f3)). Never sample `dest` while it's the target.

## I-2 · Carry-forward must preserve the 4 non-dirty bits exactly — open

The combine writes **every** pixel of `dest`; for the 4 lights that didn't move, their bits must copy
through **unchanged**. **Guard:** `dest = (src & ~bit_k) | (mask?bit_k:0)` — only bit `k` is touched; bits
0..4 except `k` are carried verbatim from `src` (float-mod: reconstruct the RED byte, clear bit `k`, OR the
mask's bit). FAIL symptom: a light moving makes *other* lights' colours flicker → the carry-forward is
dropping bits.

## I-3 · A light moving OFF a spot must CLEAR its old bit — open

If the combine only OR-ed the new coverage, a light's old shadow would linger (bit never cleared).
**Guard:** clear bit `k` from `src` **before** OR-ing the fresh `mask` (the `& ~bit_k` above). FAIL
symptom: ghost shadows trailing a moved light.

## I-4 · No alpha channel — open

Bits live in RGB (here just RED), **A held at 1** so premultiply never touches the data. The alpha channel
is not used for coverage, bits, or anything but opacity ([`rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md)).

## I-5 · Caster enumeration + in-radius cull — open

The nuke removed `standingPrims()`; re-add it (`zIndex ≥ 1`) or plant test casters ([F5](forks.md#f5)).
Only a light's **in-radius** prims should cast (a coarse distance cull), so "send only the dirty light and
the prims it casts from" is cheap. Screen-space: project the surviving prims world→screen before casting.

## I-6 · Bit ops are float-mod (ES 1.00) — open

Pixi's high-shader is GLSL ES 1.00 (no `uint`/bitwise — proven by `bitfield-rt` I-8). Set/test/clear bits
with float math on the RED byte (`mod(floor(n/exp2(b)),2.0)` to test; add/subtract `exp2(b)` to set/clear),
exact on rgba8. This is what `bitfield-rt` proved.
