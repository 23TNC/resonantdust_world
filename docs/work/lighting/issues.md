# Issues — lighting

_Problems hit + candidate solutions + what we chose + why._

---

## I1 · The 32-bit cold-shadow bitfield build produced EMPTY shadows (reverted) — 2026-07-19

**Problem.** The P2 rework of `bakeColdShadowSquare` (RGB=3 → a 32-bit `shadow-cold` bitfield, via a
4-batch scatter→combine) produced a **completely empty** `shadow-cold` — zero cold shadows in the
render (confirmed live via `overlayRT=shadow-cold`). The RGB=3 interim it replaced had worked.

**What was ruled out (this session):**
- The build DOES run + generate geometry: a diagnostic logged `nLights 1, prims 323, 8190 verts` built.
- Not the square-level light-reach cull (bypassing it changed nothing).
- Making the cold scatter/field RTs **non-premultiplied** (`alphaMode: no-premultiply-alpha`) did NOT
  fix it.

**Where it breaks (unconfirmed):** somewhere in **scatter-render → warmCombine → field → blit**. An
`extract.pixels` of the scatter RT read max 0, but extract-during-bake is unreliable, and blitting the
raw scatter to `shadow-cold` still showed nothing. Prime suspects for the fresh attempt:
- Alpha handling on the **`shadow-cold` composite** itself (not just the scratch RTs) — the bake samples
  it; if it's premultiplied and a bit-byte has A=0, the read may corrupt.
- The **combine mesh** not sampling the scatter maps as expected (uv/transform, or `texture()` vs
  `textureBit` premultiply), or `uFreshSel`/ping-pong seeding.
- The scatter **render into `coldScatterRT`** genuinely producing nothing (transform `m` mapping the
  world-space projected geometry outside the slot, vs the interim which rendered the same geometry into
  `scratchRT` and worked).

**Chosen (for now).** **Reverted** to the RGB=3 interim (`91d4363`) so cold shadows aren't at zero.
The building blocks stay. Re-attempt the bitfield with the scatter/composite alpha + the combine
sampling **isolated and unit-verified one stage at a time** (render one lane → read it back correctly →
combine → read → blit → read) before wiring the full 4-batch loop — not built end-to-end blind.

**Cost note:** this ate a very long session with much browser round-tripping and false "it works" calls
(a set bit reads as value 1/255 ≈ black, which I misread as success). Verify the *rendered result*, not
just that geometry was generated.

**Update 2026-07-19:** two follow-ups landed. (1) The **RGB=3 interim is now solid** — a buffer-grow fix
(`3442557`) made it cast *all* trees in range (was silently dropping casters), browser-verified. That's a
good fallback. (2) The **root cause is very likely the Sprite `blit()` premultiplying** the field
(`RGB × A`, `A = 0` → bits zeroed); the fix = a non-premultiply **Mesh** copy for the field blit. The
staged re-attempt (A → D, read-back each stage) is [`todo.md`](todo.md) P2. Building blocks all committed.
