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
                ; SUBFRAME (subframe-ingest P1) — which fraction of the body master is
                ; actually art, per DIRECTION. A body is widest seen front/back and narrowest in profile.
                ; Measured from the 128-px masters on disk (surface.B > 0.35), union across
                ; every variant so no variant is clipped (F8). The atlas crops to exactly this
                ; rect for all four maps, which is what registers them with each other.
                0.1484 &body.subframe.e.x set
                0.0469 &body.subframe.e.y set
                0.7109 &body.subframe.e.w set
                0.9141 &body.subframe.e.h set
                0.0781 &body.subframe.n.x set
                0.0469 &body.subframe.n.y set
                0.8516 &body.subframe.n.w set
                0.9141 &body.subframe.n.h set
                0.0703 &body.subframe.s.x set
                0.0469 &body.subframe.s.y set
                0.8594 &body.subframe.s.w set
                0.9141 &body.subframe.s.h set
                ; ── slot 1: the HEAD — part-1 files, scaled + seated on the body ────
                "thing ^prim call &head export
                "pawn/human/female &head.texture set
                #ffffff &head.tint set
                #7a6a5a &head.geoColor set
                1 &head.part set
                ; 0.5 tiles of head art (user spec) — absolute, not 0.625 × the body.
                0.5 &head.scale set
                ; HEIGHT in TILES above slot 0's anchor (the feet) to the head FRAME's
                ; centre. Tuned against the 0.8-scale body: its art runs from 0.238 to
                ; 0.938 tiles above the anchor, so the head centres just over its top.
                ;
                ; `offset.z`, NOT `offset.y` (z-positioning F5). The two look identical on screen —
                ; the renderer draws elevation as a northward shift of the same size — but they mean
                ; opposite things to the shadow system. `offset.y` would put the head at a different
                ; WORLD position, so it would cast from its own ground footprint and the pawn would
                ; throw two shadows offset by exactly this fudge. `offset.z` keeps the body's
                ; footprint and says "this is how high it sits", so the two shadows are ONE.
                0.87 &head.offset.z set
                1 &head.span set
                ; Draw-order along the VIEW's depth axis, in the pawn's own frame:
                ; +1 = toward the viewer when it faces the camera. The client negates
                ; it when the pawn faces AWAY, so this one number gives "head over the
                ; body for e/w/s, under it for n". A backpack would author -1.
                1 &head.depth set
                ; SUBFRAME (subframe-ingest P1) — which fraction of the head master is
                ; actually art, per DIRECTION. Per SLOT: a head fills a different fraction of its frame than the body does.
                ; Measured from the 128-px masters on disk (surface.B > 0.35), union across
                ; every variant so no variant is clipped (F8). The atlas crops to exactly this
                ; rect for all four maps, which is what registers them with each other.
                0.1172 &head.subframe.e.x set
                0.0391 &head.subframe.e.y set
                0.7656 &head.subframe.e.w set
                0.9219 &head.subframe.e.h set
                0.1328 &head.subframe.n.x set
                0.0391 &head.subframe.n.y set
                0.7344 &head.subframe.n.w set
                0.9297 &head.subframe.n.h set
                0.1484 &head.subframe.s.x set
                0.0391 &head.subframe.s.y set
                0.7109 &head.subframe.s.w set
                0.9297 &head.subframe.s.h set
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
                ; SUBFRAME (subframe-ingest P1) — which fraction of the body master is
                ; actually art, per DIRECTION. A body is widest seen front/back and narrowest in profile.
                ; Measured from the 128-px masters on disk (surface.B > 0.35), union across
                ; every variant so no variant is clipped (F8). The atlas crops to exactly this
                ; rect for all four maps, which is what registers them with each other.
                0.1406 &body.subframe.e.x set
                0.0391 &body.subframe.e.y set
                0.7188 &body.subframe.e.w set
                0.9219 &body.subframe.e.h set
                0.0625 &body.subframe.n.x set
                0.0469 &body.subframe.n.y set
                0.8828 &body.subframe.n.w set
                0.9141 &body.subframe.n.h set
                0.0391 &body.subframe.s.x set
                0.0469 &body.subframe.s.y set
                0.9219 &body.subframe.s.w set
                0.9062 &body.subframe.s.h set
                "thing ^prim call &head export
                "pawn/human/male &head.texture set
                #ffffff &head.tint set
                #7a6a5a &head.geoColor set
                1 &head.part set
                0.5 &head.scale set
                ; HEIGHT, not a y-fudge — see the female kind above (z-positioning F5).
                0.87 &head.offset.z set
                1 &head.span set
                ; Draw-order along the VIEW's depth axis, in the pawn's own frame:
                ; +1 = toward the viewer when it faces the camera. The client negates
                ; it when the pawn faces AWAY, so this one number gives "head over the
                ; body for e/w/s, under it for n". A backpack would author -1.
                1 &head.depth set
                ; SUBFRAME (subframe-ingest P1) — which fraction of the head master is
                ; actually art, per DIRECTION. Per SLOT: a head fills a different fraction of its frame than the body does.
                ; Measured from the 128-px masters on disk (surface.B > 0.35), union across
                ; every variant so no variant is clipped (F8). The atlas crops to exactly this
                ; rect for all four maps, which is what registers them with each other.
                0.1016 &head.subframe.e.x set
                0.0391 &head.subframe.e.y set
                0.8047 &head.subframe.e.w set
                0.9219 &head.subframe.e.h set
                0.125 &head.subframe.n.x set
                0.0469 &head.subframe.n.y set
                0.75 &head.subframe.n.w set
                0.9141 &head.subframe.n.h set
                0.1406 &head.subframe.s.x set
                0.0312 &head.subframe.s.y set
                0.7266 &head.subframe.s.w set
                0.9375 &head.subframe.s.h set
                0 return
            @on_destroy>
                0 return
