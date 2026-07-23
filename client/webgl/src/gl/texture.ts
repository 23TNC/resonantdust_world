//! Texture — a 2D GL texture with first-class support for the formats the shadow/lighting work needs:
//! `rgba8unorm` (colour + the current bitfield), `rgba32float` (light/caster data), and the INTEGER formats
//! `rgba8uint` / `rgba32uint` (the >24-bit bitfield, read via `usampler2D`). Integer + float textures use
//! NEAREST filtering (LINEAR is invalid on them). Atlas frame UVs are the caller's job (we keep the whole
//! page here); the resolver already computes frames.

export type TexFormat = "rgba8unorm" | "rgba32float" | "rgba8uint" | "rgba32uint" | "r8uint";

interface Fmt {
  internal: number;
  format: number;
  type: number;
  integer: boolean;
}

function glFmt(gl: WebGL2RenderingContext, f: TexFormat): Fmt {
  switch (f) {
    case "rgba8unorm":
      return { internal: gl.RGBA8, format: gl.RGBA, type: gl.UNSIGNED_BYTE, integer: false };
    case "rgba32float":
      return { internal: gl.RGBA32F, format: gl.RGBA, type: gl.FLOAT, integer: false };
    case "rgba8uint":
      return { internal: gl.RGBA8UI, format: gl.RGBA_INTEGER, type: gl.UNSIGNED_BYTE, integer: true };
    case "rgba32uint":
      return { internal: gl.RGBA32UI, format: gl.RGBA_INTEGER, type: gl.UNSIGNED_INT, integer: true };
    case "r8uint":
      return { internal: gl.R8UI, format: gl.RED_INTEGER, type: gl.UNSIGNED_BYTE, integer: true };
  }
}

export interface TextureOptions {
  width: number;
  height: number;
  format?: TexFormat;
  /** Initial pixel data (or null for an empty/attachable texture). */
  data?: ArrayBufferView | TexImageSource | null;
  /** false → LINEAR (only valid for rgba8unorm); default NEAREST. */
  nearest?: boolean;
  /** Premultiply RGB by alpha on a `TexImageSource` upload. Colour maps (albedo) want `true`;
   *  data maps (surface/layers/normal — exact bytes) want `false`. Default `false` (verbatim). */
  premultiply?: boolean;
}

export class Texture {
  readonly handle: WebGLTexture;
  readonly width: number;
  readonly height: number;
  readonly format: TexFormat;
  private readonly gl: WebGL2RenderingContext;
  private readonly fmt: Fmt;
  private readonly premultiply: boolean = false;

  /** A 1×1 opaque-white texture — the geo-tier residual/surface fill + the atlas white stem. */
  static white(gl: WebGL2RenderingContext): Texture {
    return new Texture(gl, { width: 1, height: 1, data: new Uint8Array([255, 255, 255, 255]) });
  }

  constructor(gl: WebGL2RenderingContext, opts: TextureOptions) {
    this.gl = gl;
    this.width = opts.width;
    this.height = opts.height;
    this.format = opts.format ?? "rgba8unorm";
    this.fmt = glFmt(gl, this.format);
    this.premultiply = opts.premultiply ?? false;
    this.handle = gl.createTexture()!;
    gl.bindTexture(gl.TEXTURE_2D, this.handle);
    // Integer/float textures can't filter LINEAR; and we sample everything NEAREST anyway.
    const filter = opts.nearest === false && !this.fmt.integer && this.format === "rgba8unorm" ? gl.LINEAR : gl.NEAREST;
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    this.allocate(opts.data ?? null);
  }

  /** (Re)allocate storage at the current size with optional data. */
  private allocate(data: ArrayBufferView | TexImageSource | null): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.handle);
    if (data && !(ArrayBuffer.isView(data))) {
      // A DOM image source (ImageBitmap/canvas): the atlas uploads sprites this way. Premultiply is
      // per-map (colour vs data); the context is premultipliedAlpha so keep the store flag explicit.
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, this.premultiply);
      gl.texImage2D(gl.TEXTURE_2D, 0, this.fmt.internal, this.fmt.format, this.fmt.type, data as TexImageSource);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    } else {
      // Tight packing so single-channel (R8UI) rows of any width upload correctly (default is 4).
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      gl.texImage2D(gl.TEXTURE_2D, 0, this.fmt.internal, this.width, this.height, 0, this.fmt.format, this.fmt.type, (data as ArrayBufferView) ?? null);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    }
  }

  /** Update the whole texture from a typed array (same size/format). */
  upload(data: ArrayBufferView): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.handle);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1); // tight rows (R8UI widths aren't 4-aligned)
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, this.width, this.height, this.fmt.format, this.fmt.type, data);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
  }

  /** Update a FULL-WIDTH row span `[rowStart, rowStart + rowCount)` straight out of the full-size
   *  CPU mirror (no repacking — rows are contiguous, `srcOffset` indexes into the mirror). ONE call
   *  per texture per frame over the dirty rows' bounding span is the cheap upload shape: per-call
   *  overhead dominates transfer cost at data-texture sizes, and a full-width span is a single
   *  driver memcpy. `elemsPerTexel` = typed-array elements per texel (e.g. 4 for RGBA32UI). */
  uploadRows(rowStart: number, rowCount: number, mirror: ArrayBufferView, elemsPerTexel: number): void {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.handle);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texSubImage2D(
      gl.TEXTURE_2D, 0, 0, rowStart, this.width, rowCount, this.fmt.format, this.fmt.type,
      mirror as ArrayBufferView<ArrayBuffer>, rowStart * this.width * elemsPerTexel,
    );
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
  }

  bind(unit: number): void {
    this.gl.activeTexture(this.gl.TEXTURE0 + unit);
    this.gl.bindTexture(this.gl.TEXTURE_2D, this.handle);
  }

  destroy(): void {
    this.gl.deleteTexture(this.handle);
  }
}
