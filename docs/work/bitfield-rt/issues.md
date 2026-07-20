# Issues — bitfield-rt

_The "ton of issues" this experiment exists to prove against — each is a way a bitfield gets corrupted
in an RT round-trip, with the guard that prevents it. Seeded on open (2026-07-20) as the checklist the
E1–E4 steps must satisfy; if a FAIL-mode appears in E4, its row here is the diagnosis. Move to
"resolved" as each is proven clean in-browser._

---

## I-1 · Premultiplied alpha zeroes data bytes — guard: integer RT (or verbatim write) — open

The classic one. A Sprite/tint batch or a premultiplied RT does `RGB × A`, so any byte where A=0 goes to
zero. **Guard:** `shadow`/`lightmap` bitfield RTs are **`RGBA8UI` integer** targets — integer targets are
not premultiplied or blended, so all 4 bytes (A included) are raw data. (Fallback if integer RTs are
fiddly in Pixi: unorm RGBA8 + the nuked build's **verbatim, non-premultiply** Mesh write.) FAIL symptom:
black/zero rects.

## I-2 · Bilinear filtering interpolates bytes into garbage bits — guard: nearest — open

Sampling a bitfield with linear filtering averages neighbouring texels' *byte values*, which is nonsense
for packed bits. **Guard:** sampler **`nearest`** on both the RT and every read; with an integer
`usampler2D`, use **`texelFetch`** (no filtering by construction). FAIL symptom: smeared/wrong colours at
rect edges.

## I-3 · sRGB / colour-space gamma-mangles the byte — guard: linear RT — open

If the RT is sRGB-encoded, writes/reads pass through gamma curves and a byte no longer round-trips as its
integer value. **Guard:** the RT is **linear** (no sRGB). Integer `RGBA8UI` sidesteps this entirely (no
colour semantics). FAIL symptom: off-by-one / wrong bit.

## I-4 · Float rounding on decode — guard: integer path, or `+0.5` — open

If the bitfield is read as a unorm float and reconstructed (`v*255`), truncation can land a bit off.
**Guard:** integer `usampler2D` → the value *is* the uint, no reconstruction. If forced to unorm, decode
`uint(v*255.0 + 0.5)`. FAIL symptom: off-by-one bit.

## I-5 · Blend state corrupts the write — guard: blend off — open

Any enabled blend (`add`/`max`/normal) combines the fragment with what's in the RT, so the written
bitfield isn't the exact value. **Guard:** **blend disabled**; each rect is written once, replace. (In
`shadows`' real pack the write is a deliberate read-modify-write OR — also blend-off — but this experiment
writes one-hot once, so plain replace.) FAIL symptom: unexpected extra/missing bits.

## I-6 · World→buffer coordinate / rect addressing — guard: reuse the cache mapping — open

Writing the right value to the right rect (and reading it back at the same world spot after a pan/zoom)
requires the exact toroidal window→slot mapping the composites use. **Guard:** derive the fill's rect
index and the overlay's sample coordinate from the **same** `SquareCache` window/slot math — don't invent
a parallel coordinate path. FAIL symptom: all-one-colour (fill mis-addressed), or colours that shift on
pan/zoom (read mis-addressed).
