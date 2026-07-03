# Resonant Dust — world

A multiplayer world game: a SpacetimeDB server backend, a shared Rust core
compiled both native (server) and to wasm (client), and a PixiJS browser client.
Per-area notes live in scoped `AGENTS.md` files (e.g. `shared/AGENTS.md`) and in
`docs/`.

## Art style — READ FIRST before touching any art

Full spec: **[docs/art-style.md](docs/art-style.md)**. The essentials:

- **Look:** RimWorld / Prison Architect — flat, simple, hand-painted, minimal cel
  shading, bold clean outlines. Native to **SDXL**; avoid photoreal models
  (FLUX-base over-renders and fights the flat look) for base generation.
- **Perspective: oblique 3/4 top-down (pseudo-isometric).** The world grid is
  top-down, but characters/objects show their **front or sides** — NOT a true
  bird's-eye overhead, NOT true isometric. Rendering a creature from directly
  above is WRONG.
- **Directional facings:** East = side profile facing right; West = mirror of
  East; **South = front view** (faces viewer); **North = back view** (faces away).
  East/West are easy (side profiles, in-distribution); South/North are hard
  (SDXL defaults to side view — needs an edit model or organic reference to turn).
- **Tint regions:** author three isolable color regions — primary (main body),
  secondary (markings), detail (eyes/trim) — as normal colored art, not RGB masks.
- **Cutout:** deliver transparent alpha cutouts. The old magenta `0xFF00FF` key is
  retired for generated art — `rembg` gives alpha directly; `bin/art` consumes it.

## Art tooling

- `bin/art` — slice/key/master sprite sheets; masters normalize to power-of-two.
- `bin/marigold` — ML maps (albedo/normal/depth) for the renderer lighting pass.
- Sprite generation (in progress): ComfyUI + SDXL + ControlNet + rembg — see the
  `sprite-gen-pipeline` memory.
