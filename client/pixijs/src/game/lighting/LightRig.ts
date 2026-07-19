//! The light rig — the viewport's set of lights (directional sun + ambient floor + up to
//! {@link MAX_HOT_LIGHTS} dynamic point lights) and the per-frame packing that uploads them
//! to the {@link LightingShader}'s uniform group.
//!
//! Adapted from the old game's `DeferredLighting.packChunkLights`, but in WORLD px with NO
//! origin subtraction: the new game lights in one screen-space pass over the toroidal
//! composite, and the display mesh already carries each fragment's world position, so a
//! light's position is just its world px — no per-chunk coordinate juggling.
//!
//! Lights are plain mutable records the owner holds a handle to: move one (a torch on a
//! walking pawn) by mutating `x`/`y` and the next {@link pack} picks it up. The cursor light
//! is a distinguished slot so a hover highlight needs no register/unregister churn.

import { MAX_HOT_LIGHTS, type LightingShader } from "../viewport/lightingShader";
import { MAX_COLD_LIGHTS, type LightingBakeShader } from "../viewport/lightingBakeShader";

/** Which lighting tier a light belongs to (docs/work/lighting). **cold** = static → summed into the
 *  per-rect baked `cold_lightmap`, effectively unlimited (amortized, dirty-rebake). **dynamic** =
 *  moving / real-time → the 32-light pool evaluated live at display, with round-robin scatter shadows.
 *  A light only ever lives in one; a light that starts moving migrates cold→dynamic. */
export type LightTier = "cold" | "dynamic";

/** A point light, positioned in WORLD px. `height` (z above the ground plane) drives the N·L term on
 *  flat terrain and the shadow projection; `radius` is the falloff cutoff. */
export interface PointLight {
  x: number;
  y: number;
  /** Height above the ground plane, world px. */
  height: number;
  /** Falloff cutoff radius, world px (intensity → 0 at the edge). */
  radius: number;
  /** `0xRRGGBB`. */
  color: number;
  /** Intensity scalar, decoupled from colour. */
  brightness: number;
  /** Whether this light casts occlusion shadows. Fill lights set false. */
  castsShadow: boolean;
  /** Its tier — see {@link LightTier}. Defaults `dynamic`. */
  tier: LightTier;
}

/** Sensible defaults for a point light, so callers spell out only what differs. */
export function pointLight(p: Partial<PointLight> & Pick<PointLight, "x" | "y">): PointLight {
  return {
    height: 96,
    radius: 4 * 64,
    color: 0xffffff,
    brightness: 1.5,
    castsShadow: true,
    tier: "dynamic",
    ...p,
  };
}

export class LightRig {
  /** Ambient floor — a flat colour added everywhere. Zeroed: nothing lifts the shadow side,
   *  so a surface facing away from the cursor light (or outside its pool) falls to pure black. */
  ambient = { color: 0x5a6478, intensity: 0.5 };
  /** The directional sun — a world/tangent-space direction (normalised at pack), colour,
   *  and intensity. Zeroed: the cursor point light is the key, not a global sun. */
  sun = { dir: { x: -0.4, y: -0.5, z: 0.75 }, color: 0xfff4e0, intensity: 0 };

  /** The registered dynamic lights (torches, glowing things). */
  private readonly lights = new Set<PointLight>();
  /** The distinguished cursor/hover light, or null when the pointer is off the world. */
  private cursor: PointLight | null = null;
  /** Debug (`?nocursorlight`): keep the cursor light permanently off, so nothing casts shadows. */
  private cursorDisabled = false;

  /** Reused scratch for the packed uniform arrays (no per-frame allocation churn). */
  private readonly data = new Float32Array(MAX_HOT_LIGHTS * 4);
  private readonly color = new Float32Array(MAX_HOT_LIGHTS * 4);
  /** Reused scratch for the COLD packing ({@link packCold}). */
  private readonly coldData = new Float32Array(MAX_COLD_LIGHTS * 4);
  private readonly coldColor = new Float32Array(MAX_COLD_LIGHTS * 4);

  /** Add a dynamic light; returns the same record so the caller can mutate/remove it. */
  register(light: PointLight): PointLight {
    this.lights.add(light);
    return light;
  }

  /** Drop a previously-registered light. */
  unregister(light: PointLight): void {
    this.lights.delete(light);
  }

  /** Position the cursor light at a world-px point, or clear it (pointer left the world).
   *  Cheap to call every pointermove — it mutates one record. */
  setCursorWorld(x: number | null, y = 0): void {
    if (x === null || this.cursorDisabled) {
      this.cursor = null;
      return;
    }
    if (!this.cursor) this.cursor = pointLight({ x, y, height: 140, radius: 6 * 64, color: 0xfff4ec, brightness: 1.4 });
    else {
      this.cursor.x = x;
      this.cursor.y = y;
    }
  }

  /** Debug: enable/disable the cursor light. Disabled clears it now and blocks re-creation, so
   *  nothing casts shadows (the shadow pass gets no caster light and just blanks); enabled lets the
   *  next pointer move re-seed it. Driven by the `/nocursorlight` command (chat or URL). */
  setCursorDisabled(disabled: boolean): void {
    this.cursorDisabled = disabled;
    if (disabled) this.cursor = null;
  }

  /** Permanently disable the cursor light — {@link setCursorDisabled}(true). */
  disableCursor(): void {
    this.setCursorDisabled(true);
  }

  /** The **cold** (static) lights — summed into the per-rect baked `cold_lightmap` (lighting P1),
   *  not evaluated live. Empty until content authors static lights (a debug light seeds P1). */
  coldLights(): PointLight[] {
    const out: PointLight[] = [];
    for (const l of this.lights) if (l.tier === "cold") out.push(l);
    return out;
  }

  /** The first ≤3 cold lights as shadow casters (`{x, y, z, radius}`), in the SAME order `packCold`
   *  packs them (so lane i in the coldShadow bake matches cold light i in the lightmap). Lighting P4. */
  coldShadowLights(): { x: number; y: number; z: number; radius: number }[] {
    const out: { x: number; y: number; z: number; radius: number }[] = [];
    for (const l of this.lights) {
      if (l.tier !== "cold" || !l.castsShadow) continue;
      out.push({ x: l.x, y: l.y, z: l.height, radius: l.radius });
      if (out.length >= 3) break;
    }
    return out;
  }

  /** Pack the cold lights into the lightmap-bake shader (lighting P1) — `data = xy,z,radius`,
   *  `color = rgb,brightness` (no shadow sign; cold shadows bake separately). Called before the cold
   *  cache's `bakeDirty` so a dirty rect re-sums the current cold set. */
  packCold(shader: LightingBakeShader): void {
    let n = 0;
    for (const l of this.lights) {
      if (l.tier !== "cold" || n >= MAX_COLD_LIGHTS) continue;
      const o = n * 4;
      this.coldData[o] = l.x;
      this.coldData[o + 1] = l.y;
      this.coldData[o + 2] = l.height;
      this.coldData[o + 3] = l.radius;
      this.coldColor[o] = ((l.color >> 16) & 0xff) / 255;
      this.coldColor[o + 1] = ((l.color >> 8) & 0xff) / 255;
      this.coldColor[o + 2] = (l.color & 0xff) / 255;
      this.coldColor[o + 3] = l.brightness;
      n++;
    }
    shader.setLights(this.coldData, this.coldColor, n);
  }

  /** The **dynamic** point lights that cast shadows (cursor first, then registered), for the
   *  scatter pass. Fill lights + cold lights (baked, not scattered) are excluded. */
  shadowCasters(): PointLight[] {
    const out: PointLight[] = [];
    if (this.cursor?.castsShadow) out.push(this.cursor);
    for (const l of this.lights) if (l.castsShadow && l.tier === "dynamic") out.push(l);
    return out;
  }

  /** Pack the sun, ambient, and up to {@link MAX_HOT_LIGHTS} point lights (cursor first)
   *  into the shader's uniforms. Called once per frame before the display draw. */
  pack(shader: LightingShader): void {
    shader.setAmbient(this.ambient.color, this.ambient.intensity);
    shader.setSun(this.sun.dir, this.sun.color, this.sun.intensity);

    let n = 0;
    const push = (l: PointLight): void => {
      if (n >= MAX_HOT_LIGHTS) return;
      const o = n * 4;
      this.data[o] = l.x;
      this.data[o + 1] = l.y;
      this.data[o + 2] = l.height;
      // Encode casts-shadow in the SIGN of the radius (shader takes |w| for falloff, reads
      // the sign as the shadow gate) — no extra uniform slot needed.
      this.data[o + 3] = l.castsShadow ? l.radius : -l.radius;
      this.color[o] = ((l.color >> 16) & 0xff) / 255;
      this.color[o + 1] = ((l.color >> 8) & 0xff) / 255;
      this.color[o + 2] = (l.color & 0xff) / 255;
      this.color[o + 3] = l.brightness;
      n++;
    };
    // The DYNAMIC pool only — cursor first, then registered dynamic lights. Cold lights are summed
    // into the baked `cold_lightmap` (lighting P1), never uploaded to the live display loop.
    if (this.cursor) push(this.cursor);
    for (const l of this.lights) if (l.tier === "dynamic") push(l);

    shader.setLights(this.data, this.color, n);
  }
}
