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

## I-7 · The experiment proves *storage*, not the *read-modify-write accumulate* `shadow-cold` needs — open, 2026-07-20

E1–E4 write each rect **once** (one-hot, blend-off replace — E2 says "read-modify-write not even needed
here"), so they prove a bitfield **survives an RT round-trip when written in a single pass**. But the
mechanism `shadows` actually depends on — and the one the [README](README.md#why) calls "the one mechanism
that has failed repeatedly" — is subtler: **OR-ing new bits into a bitfield that already holds bits, across
passes, without corrupting the existing ones.** That's a read-modify-write of a bound RT, which can't be
done in place (feedback loop) and forces **ping-pong** ([shadows I-8](../shadows/issues.md#i-8)). The
current E5 stretch ("set two bits statically") still writes once — it exercises the *decode*-side additive
combine, **not** the read-back-and-OR path. **So the experiment as scoped leaves the hard part unproven.**
**Proposed fix:** promote E5 from a static two-bit write to a real **ping-pong RMW round-trip** — pass 1
writes bit `i`; pass 2 reads that RT (source) and ORs bit `j` into the *other* RT (destination); confirm the
decode shows **both** colours. That proves storage **and** accumulate — exactly what `shadow-cold`'s pack
inherits.

**RESOLVED 2026-07-20 — storage-first, then RMW as E5.** Ship the storage-only proof (E1–E4) first, then
add the ping-pong RMW proof as **E5** (E6 = graduate). *Why not widen before E1:* RMW = storage + read-back
+ OR + ping-pong; if we jump straight to RMW and it fails we can't tell which of those broke — which
violates this experiment's whole reason for existing ("a failure has one cause"). Proving single-pass
storage first means an E5 failure is isolated to the accumulate/ping-pong path on a *known-good* storage
layer. Incremental de-risking, applied to itself.

## I-8 · Pixi's high-shader compiles GLSL ES 1.00, not ES 3.00 — FOUND + handled 2026-07-20

Executing E3 proved it. The `/overlayRT` fragment (built from `compileHighShaderGlProgram` with the stock
`localUniformBit`/`textureBit`/`roundPixelsBit`) compiled with `precision mediump float;`,
`#define in varying`, `#define finalColor gl_FragColor`, and **no `#version 300 es`** — so `uint` was an
"undeclared identifier" and the shader failed to compile (overlay drew nothing). **Pixi v8's high-shader
system emits ES 1.00 with WebGL1-compat shims even on a WebGL2 context.** ES 3.00 (`uint`, bitwise,
integer textures, MRT) is only reached by hand-writing a **raw `GlProgram` with `#version 300 es`**,
bypassing the ES-1.00-shimmed stock bits — a real cost, not free.

**Handled:** the decode uses **float-mod** bit extraction — `mod(floor(byte*255/exp2(b)), 2.0)`, exact on
rgba8 (bytes are `k/255`) — which is what proved out. This **reverses the "ES 3.00 is free" premise**
behind [F14 in shadows](../shadows/forks.md#f14) and `rendering-platform.md`; both corrected. Consequence
for `shadows`: the bitfield stays **unorm RGBA8 + float-mod + A-discipline** (A=1 → 24 bits, or the
verbatim non-premultiply write → 32), NOT integer textures — unless someone hand-writes raw ES 3.00
shaders. MRT for the many-lights scale carries the same "raw ES 3.00 shader" cost.

## I-6 · World→buffer coordinate / rect addressing — guard: reuse the cache mapping — open

Writing the right value to the right rect (and reading it back at the same world spot after a pan/zoom)
requires the exact toroidal window→slot mapping the composites use. **Guard:** derive the fill's rect
index and the overlay's sample coordinate from the **same** `SquareCache` window/slot math — don't invent
a parallel coordinate path. FAIL symptom: all-one-colour (fill mis-addressed), or colours that shift on
pan/zoom (read mis-addressed).
