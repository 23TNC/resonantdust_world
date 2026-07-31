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
; of the same sex. The fit/fat/average grouping is future character-gen input — recorded
; here so it survives (docs/work/2026-07-30-human-pawns/README.md).
;
; `scale` is the slot's PRE-ATLAS sprite scale (pawn-part-placement F4): it shrinks the ART
; inside the slot's own `span × span` frame, it does NOT resize the drawn box. So it is
; ABSOLUTE — on these `span 1` slots it reads directly as tiles of art. It must not go back
; to being a draw-time multiplier: the lighting/shadow card is sized from the def's pow2
; frame span, so a box that disagrees with `span` casts a silhouette `1/scale` too big
; (docs/work/2026-07-30-pawn-part-placement/issues.md#i6).
<thing>
    ::human_female>
        :visual>
            @on_create>
                ; ── slot 0: the BODY — boxes the carrier ────────────────────────────
                "thing ^prim call &body export
                "pawn/human/female &body.texture set
                #ffffff &body.tint set
                #7a6a5a &body.geoColor set
                ; The art masters are square 128 canvases spanning ONE tile — every
                ; variant's meta.json reads `square 128, tile_px 128, span 1`. `span`
                ; sizes the ATLAS FRAME (`frame_span · SQUARE / 2^lod`), so span 2 asked
                ; for a 256 px frame to hold 128 px of art: 4× the atlas area per part,
                ; for nothing. A standing human still draws ~1.5 tiles tall via `size` —
                ; that is magnification off a 1-tile frame, which is what `size` is for —
                ; on a 1×1 footprint, feet pinned to the cell's front edge (the wolf's
                ; pattern).
                ;
                ; The comment here previously claimed 256 canvases derived from the art
                ; (`span_from=art`). Both halves were wrong: the corpus was re-mastered to
                ; 128, and meta.json reads `span_from: "default", span_inferred: true` —
                ; nothing ever derived it. `loader.rs` reads `prims.0.span` with a default
                ; of 1.0, so these literals were the only thing making it 2.
                1 &body.span set
                ; 0.8 tiles of body art in a 1-tile frame (user spec), scaled about the
                ; sprite pivot below — so the feet stay on the anchor line.
                0.8 &body.scale set
                1.0 &body.anchor.y set
                1.0 &body.sprite_anchor.y set
                ; ── slot 1: the HEAD — part-1 files, scaled + seated on the body ────
                "thing ^prim call &head export
                "pawn/human/female &head.texture set
                #ffffff &head.tint set
                #7a6a5a &head.geoColor set
                1 &head.part set
                ; 0.5 tiles of head art (user spec) — absolute, not 0.625 × the body.
                0.5 &head.scale set
                ; Offset in TILES from slot 0's anchor (the feet) to the head FRAME's
                ; centre. Tuned against the 0.8-scale body: its art runs from 0.238 to
                ; 0.938 tiles above the anchor, so the head centres just over its top.
                -0.87 &head.offset.y set
                1 &head.span set
                ; Draw-order along the VIEW's depth axis, in the pawn's own frame:
                ; +1 = toward the viewer when it faces the camera. The client negates
                ; it when the pawn faces AWAY, so this one number gives "head over the
                ; body for e/w/s, under it for n". A backpack would author -1.
                1 &head.depth set
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
                1 &body.span set
                0.8 &body.scale set
                1.0 &body.anchor.y set
                1.0 &body.sprite_anchor.y set
                "thing ^prim call &head export
                "pawn/human/male &head.texture set
                #ffffff &head.tint set
                #7a6a5a &head.geoColor set
                1 &head.part set
                0.5 &head.scale set
                -0.87 &head.offset.y set
                1 &head.span set
                ; Draw-order along the VIEW's depth axis, in the pawn's own frame:
                ; +1 = toward the viewer when it faces the camera. The client negates
                ; it when the pawn faces AWAY, so this one number gives "head over the
                ; body for e/w/s, under it for n". A backpack would author -1.
                1 &head.depth set
                0 return
            @on_destroy>
                0 return
