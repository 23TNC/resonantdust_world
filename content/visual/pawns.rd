; Human pawn visuals (human-pawns P2) — the first MULTI-PART pawns. A pawn's visual is a
; PARTS LIST: every `^prim call` in @on_create declares one part SLOT, in order (slot 0 =
; body, slot 1 = head — matching the art's `<map>.<dir>.<part>` filenames). The DSL is the
; SKELETON only: slots, scales, offsets. WHAT each slot draws (which body/head variant —
; or, later, an armor def replacing a slot) rides the pawn's payload (`PART(slot, def)`,
; TABLES.md §payload); a slot with no payload entry draws the pawn's own def.
;
; Body variants (both sexes, 9 each, `pawn/human/<sex>/<variant>/…0.png`):
;   fit = 0, 1, 2  ·  fat = 3, 4, 5  ·  average = 6, 7, 8
; Head variants (both sexes, 16 each, part-1 files, folders 0..15): any head fits any body
; of the same sex. Head canvases are held about body size, so the head slot carries
; `scale 0.625` (user spec). The fit/fat/average grouping is future character-gen input —
; recorded here so it survives (docs/work/2026-07-30-human-pawns/README.md).
<thing>
    ::human_female>
        :visual>
            @on_create>
                ; ── slot 0: the BODY — boxes the carrier ────────────────────────────
                "thing ^prim call &body export
                "pawn/human/female &body.texture set
                #ffffff &body.tint set
                #7a6a5a &body.geoColor set
                ; The art masters are square 256 canvases spanning 2 tiles (meta.json
                ; span_from=art); a standing human draws ~1.5 tiles tall on a 1×1
                ; footprint, feet pinned to the cell's front edge (the wolf's pattern).
                1.5 &body.size set
                2 &body.span set
                1.0 &body.anchor.y set
                1.0 &body.sprite_anchor.y set
                ; ── slot 1: the HEAD — part-1 files, scaled + seated on the body ────
                "thing ^prim call &head export
                "pawn/human/female &head.texture set
                #ffffff &head.tint set
                #7a6a5a &head.geoColor set
                1 &head.part set
                ; 0.625 × the body's drawn size (user spec — head canvases are held
                ; about body size).
                0.625 &head.scale set
                ; Offset in TILES from slot 0's anchor (the feet): the head canvas
                ; centres ~1.15 tiles up. First-guess placement — the P4 drill tunes it
                ; with the user's eyes.
                -1.15 &head.offset.y set
                2 &head.span set
                0 return
            @on_destroy>
                0 return
    ::human_male>
        :visual>
            @on_create>
                "thing ^prim call &body export
                "pawn/human/male &body.texture set
                #ffffff &body.tint set
                #7a6a5a &body.geoColor set
                1.5 &body.size set
                2 &body.span set
                1.0 &body.anchor.y set
                1.0 &body.sprite_anchor.y set
                "thing ^prim call &head export
                "pawn/human/male &head.texture set
                #ffffff &head.tint set
                #7a6a5a &head.geoColor set
                1 &head.part set
                0.625 &head.scale set
                -1.15 &head.offset.y set
                2 &head.span set
                0 return
            @on_destroy>
                0 return
