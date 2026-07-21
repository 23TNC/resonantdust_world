//! Font-loading. Registers the app's font faces in `document.fonts` (via the
//! `FontFace` API) before first render so text rasterises with the real font, not
//! a fallback. The pixijs client also baked a Pixi `BitmapFont` here for canvas
//! card-title text; in the webgl client text is DOM ([F4](../../../docs/work/webgl-engine/forks.md)),
//! so that bake is gone — this is now pure `FontFace` registration.

/** Per-face spec. `weight` follows CSS numeric weights so the browser picks the
 *  right face for a given `font-weight`. */
const FONTS: { family: string; url: string; weight: string }[] = [
  { family: "Noto Emoji", url: "/fonts/NotoEmoji/NotoEmoji-Light.ttf", weight: "300" },
  { family: "Noto Emoji", url: "/fonts/NotoEmoji/NotoEmoji-Regular.ttf", weight: "400" },
  { family: "Noto Emoji", url: "/fonts/NotoEmoji/NotoEmoji-Medium.ttf", weight: "500" },
  { family: "Noto Emoji", url: "/fonts/NotoEmoji/NotoEmoji-SemiBold.ttf", weight: "600" },
  { family: "Noto Emoji", url: "/fonts/NotoEmoji/NotoEmoji-Bold.ttf", weight: "700" },
];

/** Family name to use for emoji glyphs (gear, toolbar icons). */
export const NOTO_EMOJI_FAMILY = "Noto Emoji";

let loadPromise: Promise<void> | null = null;

/** Idempotent. Resolves once every font in `FONTS` is loaded and registered.
 *  Awaited once at bootstrap so text has its real face on first paint. */
export function loadFonts(): Promise<void> {
  if (loadPromise) return loadPromise;
  loadPromise = (async () => {
    await Promise.all(
      FONTS.map(async ({ family, url, weight }) => {
        const face = new FontFace(family, `url(${url})`, { weight });
        await face.load();
        document.fonts.add(face);
      }),
    );
  })();
  return loadPromise;
}
