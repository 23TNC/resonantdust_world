# Sprite generation — plan

How we generate directional creature/pawn sprites, given what we proved and the
corrected perspective (see [art-style.md](art-style.md)). Backend is ComfyUI on
the unraid box, driven over its HTTP API; Claude does prompt authoring + visual QA.

## What we proved (grounding)

- **SDXL nails the flat look** — the fox and the East wolves are the target
  quality straight out of the model. FLUX-base is not needed and hurts (over-renders).
- **rembg → clean alpha cutout.** Magenta retired. `bin/art` consumes alpha.
- **ControlNet follows *organic* edges faithfully** (the fox-in-wolf-pose test).
  **Crude geometric templates get traced into a blob** — never use primitives.
- **Perspective:** E/W = side profile (in-distribution, *easy*); S/N = front/back
  (*hard* — SDXL's side-view prior resists turning the subject).
- **Turbo is high-variance** → batch several and curate; single-gen is a coin flip.

## Primary approach: reference-driven mutation (validated 2026-07-03)

The strongest approach found. Instead of making SDXL invent perspective/pose from
a text prompt, **supply a correct reference asset and mutate its species.**

```
REFERENCE asset (correct facing+perspective+style)     ← authored by hand / ChatGPT
   │  extract edges (organic — the fox/lion test)
   ▼
ControlNet structure-transfer + "<species>" prompt  ──►  new creature,
   │                                                     SAME pose/perspective/style
   ▼  rembg
transparent sprite
```

- **Validated:** one east-wolf reference → clean cat and lion sprites locked to the
  wolf's stance, perspective, and flat style. **ControlNet structure-transfer wins
  decisively; latent img2img (asset as init) is washed-out and drifts** — use
  ControlNet on the reference's edges, not img2img.
- **Why it's the answer:** perspective + style are *inherited from the reference*,
  so SDXL never has to invent them. This **dissolves the South/North problem** —
  author one correct front and one back reference by hand; every creature inherits
  correct S/N. The thing the model couldn't invent, the reference provides.
- **Reference library:** a handful of references per **body-plan** (canid, small
  feline, biped/humanoid, bird…) × facing (E/S/N; W = mirror). Authored once,
  reused for the whole bestiary.
- **Caveat — silhouette lock:** ControlNet preserves the reference outline, so
  mutating across very different body-plans distorts proportions (wolf→lion great;
  wolf→cat came out leggy). Mitigate: (a) references per body-plan so shapes start
  close; (b) lower ControlNet strength (~0.5) for more re-proportioning freedom.
- **Tuning knobs:** ControlNet strength (~0.5 loose ↔ ~0.8 tight), end_percent,
  optional IP-Adapter for palette/style steering; batch + Claude-vision QA as below.

The anchor-and-derive strategy below still applies *within a body-plan* when you
don't yet have a reference for a facing; reference-mutation is preferred when a
reference exists (which, going forward, it will).

## Strategy: anchor-and-derive

Don't generate four independent facings and hope they match. **Anchor on one hero
frame, derive the rest** — this is what keeps a set looking like *one* creature.

```
desc ──► EAST hero (side profile)         ← the identity anchor
          │
          ├─► WEST   = horizontal mirror of East          (free, perfect match)
          ├─► SOUTH  = front view, identity from East      (hard — see method)
          └─► NORTH  = back view,  identity from East      (hard — see method)
```

### Per-facing method

| Facing | Method | Difficulty |
| --- | --- | --- |
| **East** | SDXL prompt-only, batch → QA-curate the best clean right-facing profile | Easy |
| **West** | Mirror East horizontally (flag asymmetric markings for touch-up) | Trivial |
| **South** | Front view — identity locked to East, pose forced (see "cracking S/N") | Hard |
| **North** | Back view — same as South | Hard |

## Cracking S/N (the one real unknown)

SDXL won't turn the animal on prompt alone. Three candidate mechanisms — we run a
**bake-off** on the wolf and pick the winner before committing:

1. **Edit-model rotation** — feed the East hero to **Qwen-Image-Edit** (half-staged
   on the box; needs the transformer) or FLUX-Kontext: *"the same wolf, viewed from
   the front / behind."* Purpose-built for view-change with identity preservation.
   Best theoretical fit; cost is the install + slower gen.
2. **Organic pose-reference ControlNet + IP-Adapter identity** — a small library of
   *real* (never geometric) front/back pose references per body-plan; ControlNet
   forces the facing, IP-Adapter (identity from the East hero) keeps it the same
   creature. Most reusable across all creatures once the reference library exists.
3. **Independent gen + color-match** — batch S/N with strong front/back prompts,
   curate the rare good ones, palette-transfer to match East. Weakest identity;
   fallback only.

**Lean:** (1) if the install pays off, (2) as the robust production default. The
pose-reference library in (2) is the correct version of the failed "template"
idea — organic references, not ellipses.

## Quality upgrades (apply throughout)

- **Checkpoint:** move off Turbo for finals. Source a **stylized SDXL checkpoint**
  (illustration / game-art) or SDXL-base + a flat-art style LoRA; run ~25–30 steps.
  Keep Turbo for fast previews/iteration. (Box also has `moodyAnimaMix` /
  `moodyProMix` UNETs worth trying.)
- **IP-Adapter** (installed) — the identity carrier from East hero → S/N.
- **Claude-vision QA gate** — automated per candidate:
  *"Single creature? Correct facing for this frame? Clean silhouette (no extra
  limbs/heads/duplicates)? On-style (flat, not photoreal)?"* → score, keep best,
  retry the frame if none pass. This is what makes batch-and-curate hands-off.
- **Normalization/assembly** (PIL): unify canvas size, center, match scale across
  the 4 frames, ground-anchor, trim. Output transparent alpha PNGs.

## Target CLI: `bin/sprite`

```
bin/sprite gen "<vague description>" <category>      # e.g. pawn/wolf

 ① Claude(Sonnet): desc → identity spec (anatomy, colors, primary/secondary/detail regions)
 ② EAST hero:  SDXL batch → rembg → Claude-vision QA → pick hero
 ③ WEST:       mirror(east)
 ④ SOUTH/NORTH: winning S/N method, identity=east hero → rembg → QA
 ⑤ Assemble:   normalize/center/scale-match 4 frames → transparent PNGs
 ⑥ Handoff:    write to textures/sprites/<category>/{e,w,s,n}.diffuse.png
               → bin/art (slice/master) → bin/marigold (albedo/normal/depth)
```

Repo-side driver, model-on-the-box — same split as the world server. The ComfyUI
workflow JSON is per-step, so the S/N mechanism and the checkpoint are swappable
config, not rewrites.

## Phased rollout (immediate → tool)

1. **De-risk S/N** — run the bake-off (methods 1–3) on the wolf; lock the method.
   *This is the next action and gates everything else.*
2. **Lock identity** — wire IP-Adapter East→S/N; confirm one coherent wolf across
   all four facings.
3. **Quality pass** — swap in a stylized checkpoint; re-shoot the wolf set at
   final quality.
4. **Automate QA** — implement the Claude-vision gate + retry loop.
5. **Scaffold `bin/sprite`** — wire ①–⑥ end to end; prove on a second creature
   from a cold description.
6. **Body-plan reference library** — organic pose references for quadruped +
   humanoid pawn, so new creatures reuse them.
```
