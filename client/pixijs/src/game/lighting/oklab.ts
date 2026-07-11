//! OKLab ↔ sRGB GLSL helpers, as a string spliced into shaders that manipulate colour
//! perceptually. OKLab separates LIGHTNESS (L) from chroma (a,b), which is exactly the
//! axis the material system needs: perturb hue/chroma while holding L fixed, so the
//! variation reads as pigment, not light. Björn Ottosson's coefficients
//! (https://bottosson.github.io/posts/oklab/); the sRGB transfer functions are the
//! standard piecewise curve. Input/output RGB is GAMMA sRGB in 0..1 (what our albedo and
//! DSL tints are stored as), converted to/from linear internally.

export const OKLAB_GLSL = /* glsl */ `
vec3 srgbToLinear(vec3 c) {
  return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c));
}
vec3 linearToSrgb(vec3 c) {
  c = max(c, 0.0);
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c));
}
// linear sRGB → OKLab (L, a, b).
vec3 linearToOklab(vec3 c) {
  float l = 0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b;
  float m = 0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b;
  float s = 0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b;
  float l_ = pow(max(l, 0.0), 1.0 / 3.0);
  float m_ = pow(max(m, 0.0), 1.0 / 3.0);
  float s_ = pow(max(s, 0.0), 1.0 / 3.0);
  return vec3(
    0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
    1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
    0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_
  );
}
// OKLab → linear sRGB.
vec3 oklabToLinear(vec3 lab) {
  float l_ = lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z;
  float m_ = lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z;
  float s_ = lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z;
  float l = l_ * l_ * l_;
  float m = m_ * m_ * m_;
  float s = s_ * s_ * s_;
  return vec3(
     4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
  );
}

// Perturb a GAMMA-sRGB colour's HUE and CHROMA (never its L), returning gamma sRGB.
//   nHue/nChroma ∈ 0..1 (noise); hueSwing in RADIANS; chromaSwing absolute; bias ∈ -1..1
// nHue/nChroma of 0.5 (a missing noise field) → zero offset → the colour is unchanged.
vec3 jitterHueChroma(vec3 gammaRgb, float nHue, float nChroma, float hueSwing, float chromaSwing, float bias) {
  vec3 lab = linearToOklab(srgbToLinear(gammaRgb));
  float C = length(lab.yz);
  float H = atan(lab.z, lab.y);
  // Warm↔cool bias leans the (symmetric) hue swing; chroma swing is symmetric about 0.
  H += hueSwing * ((nHue - 0.5) * 2.0 + bias * 0.5);
  C = max(C + chromaSwing * (nChroma - 0.5) * 2.0, 0.0);
  vec3 jittered = vec3(lab.x, C * cos(H), C * sin(H));
  return linearToSrgb(oklabToLinear(jittered));
}
`;
