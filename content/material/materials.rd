; Materials — the render registry that turns a prim's PACKED-map channels into
; colour VARIATION (not light). Each packed channel holds a per-pixel weight; a
; prim binds a channel to a material here + a base tint, and the albedo bake pass
; perturbs that tint's HUE and CHROMA (never lightness) by a tiling noise field.
; Lightness variation reads as light; hue/chroma variation reads as material — so
; this enriches flat albedos without touching the lighting engine. See
; docs/lighting.md.
;
; A material sits directly under its `::name` (no facet, like a biome) and sets:
;   noiseField   — a tiling character field (strand|mottle|speckle|vein|grain|clump);
;                  the client resolves the name to a noise-atlas index.
;   hueSwing     — hue rotation amplitude at full noise, in DEGREES.
;   chromaSwing  — chroma perturbation amplitude at full noise (~0..0.1).
;   warmCoolBias — warm↔cool asymmetry of the swing, -1..1 (cool..warm).
;   sampleSpace  — uv (rides the sprite; default) | world (pinned to the ground).
;
; Zero swings = identity (smooth tintable), i.e. today's flat behaviour. A prim
; references a material per channel:  "mottle &tile.packed.0.material set
;                                     #6b6b6b &tile.packed.0.tint set

<material>
    ; Mid-frequency soft blotches — "pigment isn't uniform". The workhorse for
    ; stone, dirt, leather, weathered wood, skin. World-sampled so ground reads as
    ; terrain the sprites sit on rather than each tile's own private pattern.
    ::mottle>
        @on_create>
            mottle &noiseField set
            10 &hueSwing set
            0.03 &chromaSwing set
            0.2 &warmCoolBias set
            world &sampleSpace set
            0 return

    ; Fine directional clumped fibres — pine needles, fur, grass, thatch. Rides
    ; the sprite (uv) so the variation belongs to the object.
    ::strand>
        @on_create>
            strand &noiseField set
            32 &hueSwing set
            0.12 &chromaSwing set
            -0.4 &warmCoolBias set
            uv &sampleSpace set
            0 return

    ; Pine needles (material-system P5) — the conifer's FOLIAGE channel (layer 0/R).
    ; The `needle` field (short sharpened dashes, strongly stretched) carries BOTH the
    ; normal detail (RNM at bake — de-plastics the generated normal) and, via the
    ; detail-keyed colour placement (F1 lean), the green variation: colour clumps ARE
    ; needle clumps. Swings calmer than `strand` — variation should read as foliage,
    ; not paint.
    ::pineneedle>
        @on_create>
            needle &noiseField set
            14 &hueSwing set
            0.05 &chromaSwing set
            -0.2 &warmCoolBias set
            uv &sampleSpace set
            needle &detailField set
            0.6 &detailAmp set
            3 &detailScale set
            0 return

    ; Bark (material-system P5) — the conifer's TRUNK channel (layer 1/G). Mild mottled
    ; relief, NO hue swing (bark keeps its colour; only its surface roughens).
    ::bark>
        @on_create>
            mottle &noiseField set
            0 &hueSwing set
            0.03 &chromaSwing set
            0 &warmCoolBias set
            uv &sampleSpace set
            mottle &detailField set
            0.25 &detailAmp set
            1.5 &detailScale set
            0 return
