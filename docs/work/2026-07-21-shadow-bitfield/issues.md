# Issues — 2026-07-21-shadow-bitfield

_Problems hit + how resolved._

## I-1 · False line along the shadow's (transparent) bottom edge — 2026-07-21 (resolved)

The 4-corner quad's bottom edge sits at sprite **UV y = 1.0**; the silhouette mask sampled the surface
page at `frame.xy + uv*frame.wh`, i.e. exactly the **frame boundary**. With the sprites packed in a
shared atlas, sampling at the boundary reads the texel **one row past the frame** (the neighbouring
frame / padding) — often opaque — so the transparent sprite bottom lit up as a full-width red line
across each shadow. **Fix:** sample **texel centres** — `(frame.xy + 0.5 + uv*(frame.wh - 1))/ATLAS`
(new `cover()` helper) — so uv=0/1 map to the first/last *valid* texel, never the edge. Verified: the
lines are gone; only the real (narrow) trunk-base shadow remains.

Sub-snag: the fix comment used backticks around identifiers, which **closed the `/* glsl */` template
literal** (the recurring foot-gun) → a TS parse error. Removed the backticks. [[glsl-backtick-in-template-literal]]