; Thing visuals — a tinted `thing` prim per flora type. Cold-zone things aren't
; drawn client-side yet (only tiles and object-shard free things are), so these
; are authored ahead of that renderer landing, in the same prim+tint shape tiles
; use.
<thing>
    ::tree>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                ; The conifer master (master/world/conifer/1.e.0.1.albedo.png);
                ; the gate resolves the "world/conifer stem and derives the preview.
                ; tint stays white so it doesn't recolour the art; geoColor is the
                ; flat green silhouette shown until the sprite streams in.
                "biome-thing/default/conifer &thing.texture set
                #ffffff &thing.tint set
                #2f4a2a &thing.geoColor set
                ; size is the SQUARE sprite scale in TILES (default 1). 2 → the conifer
                ; draws 2 tiles tall on its 1×1 footprint, so it rises PAST its cell and
                ; we can see how overlapping things z-order (anchor row, bottom-of-screen
                ; wins). anchor + sprite_anchor y=1 pin the trunk base to the cell's front
                ; edge (bottom-centre), so the extra height grows UP. x stays centred (0.5).
                2 &thing.size set
                ; def-frame-anchors P5: the frame spans 2×2 tiles (pow2; whole px/unit).
                2 &thing.span set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                ; SUBFRAME (subframe-ingest P1/F9) — the MINIMUM bbox, PER VARIANT.
                ; A conifer varies enormously by variant: the smallest sapling fills 0.198 of its frame,
                ; the largest 0.525. One union rect would waste 2.6x the texels on the small ones.
                ; The rotation index IS the variant for a cold thing (F10): one 0..15 space serves
                ; sprite facings, linked autotile cells and variants alike.
                ; GENERATED — `python3 bin/lib/subframe.py <stem>`. Do not hand-edit: measured from
                ; the 128-px masters (surface.B > 0.35, the shader's own constant), and a number
                ; edited by hand is how an authored rect silently stops matching its art (I6).
                ; The non-indexed rect is the UNION of every variant — the fallback for an index
                ; past the mastered set, which must never clip art it does not know about.
                0.2188 &thing.subframe.x set
                0.0391 &thing.subframe.y set
                0.5625 &thing.subframe.w set
                0.9336 &thing.subframe.h set
                0.3359 &thing.subframe.r0.x set
                0.2109 &thing.subframe.r0.y set
                0.3359 &thing.subframe.r0.w set
                0.5898 &thing.subframe.r0.h set
                0.2969 &thing.subframe.r1.x set
                0.1445 &thing.subframe.r1.y set
                0.4102 &thing.subframe.r1.w set
                0.7148 &thing.subframe.r1.h set
                0.2578 &thing.subframe.r2.x set
                0.0898 &thing.subframe.r2.y set
                0.4844 &thing.subframe.r2.w set
                0.8281 &thing.subframe.r2.h set
                0.2891 &thing.subframe.r3.x set
                0.1445 &thing.subframe.r3.y set
                0.4258 &thing.subframe.r3.w set
                0.7188 &thing.subframe.r3.h set
                0.25 &thing.subframe.r4.x set
                0.0664 &thing.subframe.r4.y set
                0.5039 &thing.subframe.r4.w set
                0.875 &thing.subframe.r4.h set
                0.2188 &thing.subframe.r5.x set
                0.0391 &thing.subframe.r5.y set
                0.5625 &thing.subframe.r5.w set
                0.9336 &thing.subframe.r5.h set
                0.25 &thing.subframe.r6.x set
                0.1055 &thing.subframe.r6.y set
                0.5 &thing.subframe.r6.w set
                0.8008 &thing.subframe.r6.h set
                0.2461 &thing.subframe.r7.x set
                0.0664 &thing.subframe.r7.y set
                0.5117 &thing.subframe.r7.w set
                0.8711 &thing.subframe.r7.h set
                0.2227 &thing.subframe.r8.x set
                0.082 &thing.subframe.r8.y set
                0.5547 &thing.subframe.r8.w set
                0.8438 &thing.subframe.r8.h set
                ; The conifer's albedo splits into ch0 = foliage + ch1 = trunk (see `art
                ; split_layers world/conifer`). The canonical reconstruction is
                ;   out = packed_residual + packed.R*tint0 + packed.G*tint1
                ; so BOTH channels must set their base-colour tint (else that region's
                ; colour, subtracted into the residual, is lost). TINT-ONLY by user call
                ; (lighting-visual P5): no material binding — no hue/chroma jitter on the
                ; albedo, no RNM detail on the normal; the reconstruction is exact under
                ; identity tint and the baked normal stays the smooth generated one.
                #46d64f &thing.packed.0.tint set
                #b0754f &thing.packed.1.tint set
                0 return
            @on_destroy>
                &thing.destroy call drop
                0 return
    ::shrub>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "white &thing.texture set
                #5a6e3a &thing.tint set
                ; Small ground scatter: half-tile sprite (preserving the old host default),
                ; bottom-anchored so it sits on its cell's front edge like the taller things.
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                ; SUBFRAME (subframe-ingest P1/F9) — the MINIMUM bbox, PER VARIANT.
                ; Flora varies less (0.533..0.710), but it is the stem whose halo found the original bug
                ; (docs/work/2026-08-02-normal-frames/issues.md#i8), so it gets the same treatment.
                ; The rotation index IS the variant for a cold thing (F10): one 0..15 space serves
                ; sprite facings, linked autotile cells and variants alike.
                ; GENERATED — `python3 bin/lib/subframe.py <stem>`. Do not hand-edit: measured from
                ; the 128-px masters (surface.B > 0.35, the shader's own constant), and a number
                ; edited by hand is how an authored rect silently stops matching its art (I6).
                ; The non-indexed rect is the UNION of every variant — the fallback for an index
                ; past the mastered set, which must never clip art it does not know about.
                0.0547 &thing.subframe.x set
                0.1094 &thing.subframe.y set
                0.8984 &thing.subframe.w set
                0.7969 &thing.subframe.h set
                0.0859 &thing.subframe.r0.x set
                0.1562 &thing.subframe.r0.y set
                0.8359 &thing.subframe.r0.w set
                0.7188 &thing.subframe.r0.h set
                0.0703 &thing.subframe.r1.x set
                0.1328 &thing.subframe.r1.y set
                0.8672 &thing.subframe.r1.w set
                0.7578 &thing.subframe.r1.h set
                0.0859 &thing.subframe.r2.x set
                0.125 &thing.subframe.r2.y set
                0.8281 &thing.subframe.r2.w set
                0.7656 &thing.subframe.r2.h set
                0.0703 &thing.subframe.r3.x set
                0.125 &thing.subframe.r3.y set
                0.8672 &thing.subframe.r3.w set
                0.7578 &thing.subframe.r3.h set
                0.0781 &thing.subframe.r4.x set
                0.1406 &thing.subframe.r4.y set
                0.8516 &thing.subframe.r4.w set
                0.7422 &thing.subframe.r4.h set
                0.0703 &thing.subframe.r5.x set
                0.1406 &thing.subframe.r5.y set
                0.875 &thing.subframe.r5.w set
                0.7344 &thing.subframe.r5.h set
                0.0938 &thing.subframe.r6.x set
                0.1406 &thing.subframe.r6.y set
                0.8125 &thing.subframe.r6.w set
                0.7422 &thing.subframe.r6.h set
                0.0781 &thing.subframe.r7.x set
                0.1641 &thing.subframe.r7.y set
                0.8438 &thing.subframe.r7.w set
                0.6875 &thing.subframe.r7.h set
                0.0859 &thing.subframe.r8.x set
                0.1406 &thing.subframe.r8.y set
                0.8281 &thing.subframe.r8.w set
                0.7422 &thing.subframe.r8.h set
                0.0547 &thing.subframe.r9.x set
                0.1094 &thing.subframe.r9.y set
                0.8906 &thing.subframe.r9.w set
                0.7969 &thing.subframe.r9.h set
                0.0547 &thing.subframe.r10.x set
                0.2109 &thing.subframe.r10.y set
                0.8984 &thing.subframe.r10.w set
                0.5938 &thing.subframe.r10.h set
                0.0703 &thing.subframe.r11.x set
                0.1641 &thing.subframe.r11.y set
                0.8594 &thing.subframe.r11.w set
                0.6719 &thing.subframe.r11.h set
                0.0781 &thing.subframe.r12.x set
                0.1484 &thing.subframe.r12.y set
                0.8516 &thing.subframe.r12.w set
                0.7266 &thing.subframe.r12.h set
                0 return
            @on_destroy>
                0 return
    ::cactus>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "white &thing.texture set
                #3e6b3a &thing.tint set
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                0 return
            @on_destroy>
                0 return
    ::reed>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "white &thing.texture set
                #6f7d3a &thing.tint set
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                0 return
            @on_destroy>
                0 return
    ::rock>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                ; Placeholder: a gray square primitive (the `white` 1×1 tinted gray),
                ; NOT the wall material. `rock` was a stand-in that borrowed
                ; `linked/wall.smooth` because it was the only mastered linked kind;
                ; the wall materials now live under `biome-tile/` (their real home) and
                ; `rock` is intentionally just a gray square until real rock art exists.
                "white &thing.texture set
                #7a7a7a &thing.tint set
                #7a7a7a &thing.geoColor set
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                0 return
            @on_destroy>
                0 return
    ; Ground-cover flora — uses the `world/flora` master (30 variants; the gate
    ; resolves the "world/flora stem + derives the preview). Small: a half-tile sprite,
    ; well within its cell (unlike the 2-tile conifer). White tint keeps the art's own
    ; colours; geoColor is the flat green silhouette shown until the sprite streams in.
    ::flora>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "biome-thing/default/flora &thing.texture set
                #ffffff &thing.tint set
                #4a7a3a &thing.geoColor set
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                0 return
            @on_destroy>
                0 return
    ; Wolf — the first pawn (a bot-driven wandering mobile entity, not a scattered
    ; cold thing). Uses the mastered `pawns.animal/wolf` sprite set: e/s/n facings
    ; (west = flipped east), 5 variants; the client picks a facing from the entity's
    ; rotation and a variant from its id. White tint keeps the art's own colours;
    ; geoColor is the gray silhouette shown until the sprite streams in. Sized a bit
    ; under one tile.
    ::wolf>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "pawn/animal/wolf &thing.texture set
                #ffffff &thing.tint set
                #6a6a6a &thing.geoColor set
                ; `size` is the SQUARE sprite scale in TILES. Masters are square canvases
                ; shared across the wolf's e/s/n facings at ONE common scale (bin/art kind-
                ; normalization), so the longest facing (the side view) ~fills the canvas and
                ; the others sit proportionally smaller — one size scales all three together.
                ; 1.125 ≈ a ~1.1-tile-long wolf (front/back come out narrower + a touch
                ; shorter, as they should). anchor + sprite_anchor y=1 pin the feet to the
                ; cell's front edge; a west facing mirrors the sprite pivot x with the art.
                1.125 &thing.size set
                1.125 &thing.sprite_scale.w set
                1.125 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                ; NO SUBFRAME — deliberately (subframe-ingest I10). The wolf's rects are per FACING,
                ; and a facing is not an index: it is already in the STEM (`pawn/animal/wolf/e`).
                ; Authoring them here put the SOUTH rect on the EAST art and cut the wolf in half.
                ; Restore once facings key the stem and indices key the cell/variant.
                0 return
            @on_destroy>
                0 return
    ; Torch — the world's FIRST light-emitting kind. The lighting model is dense
    ; AUTHORED point lights, not a sun: `flora` briefly carried a light and was reverted
    ; because ~230 of them read as an ambient wash. So this is scattered sparsely and
    ; each one is meant to be individually visible.
    ;
    ; `&thing.light.reach` is the DISCRIMINATOR — a kind that never sets it emits nothing
    ; and its whole light struct stays None, which is why every other kind is untouched.
    ; The client turns this into a light LEAF carried by the billboard's own carrier prim
    ; (`WorldBridge.lightFor` → `Primitive.light`), so the sprite and its emission share
    ; one resolved position and move together.
    ::torch>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                ; No dedicated torch art yet — borrow the gray placeholder square so the
                ; emitter has a visible body. Swap to real art when it exists.
                "white &thing.texture set
                #ffd9a0 &thing.tint set
                #ffd9a0 &thing.geoColor set
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                ; Warm flame colour. `reach` is in TILES — 16 is the storage lane's max (u4
                ; biased, lighting-correctness P1) and the user's spec (2026-07-31: "we need
                ; reach 16 lights"). The old fps-table lore that pinned this at 8 measured the
                ; DELETED gather; the rework's chain was measured at 16 lights × reach 16 in
                ; ~10 ms full-chain, so 16 is affordable now. Still the cost dial: registration
                ; goes as reach², so keep torches sparse.
                1.0 &thing.light.r set
                0.85 &thing.light.g set
                0.55 &thing.light.b set
                1.0 &thing.light.intensity set
                16.0 &thing.light.reach set
                0.35 &thing.light.radius set
                ; HEIGHT IS SHADOW-CRITICAL, not just a look. `shadowCover` projects the caster's card
                ; top (elevation Zt = H·sin(WORLD_TILT)) from the light onto the ground; when the light
                ; sits BELOW that top, k = Lz/(Lz−Zt) goes negative and the caster returns 0 — no shadow
                ; at all, silently. A 2-tile tree tops out at 32·sin55° ≈ 26 units, so a light must clear
                ; that to cast against anything. 2.5 tiles = 40 units is the documented contract
                ; (shadowGather.ts:61) chosen precisely to sit above the tree billboard. An authored
                ; flame height (0.6) zeroed EVERY shadow in the world — see I37.
                2.5 &thing.light.height set
                1 &thing.light.cast set
                0 &thing.light.hot set
                1 &thing.light.flicker set
                0 return
            @on_destroy>
                &thing.destroy call drop
                0 return
    ; Blue torch — identical geometry to `torch`, cold colour. A SEPARATE KIND rather than a
    ; variant because `&thing.light.*` is authored per kind: the light struct is resolved once
    ; from the kind's visual script, so two colours cannot come from one kind without
    ; per-instance light data, which the prim leaf does not carry today. Appended AFTER
    ; `torch` so no existing object_id renumbers (the same rule that put `torch` after `wolf`).
    ; If per-instance tinting is ever wanted, the `variant` nibble of `kind_reference`
    ; (kind_id << 4 | variant) is the field to grow into — this kind is the cheap route, not
    ; the principled one.
    ::torch_blue>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "white &thing.texture set
                #a0c8ff &thing.tint set
                #a0c8ff &thing.geoColor set
                0.5 &thing.size set
                0.5 &thing.sprite_scale.w set
                0.5 &thing.sprite_scale.h set
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                ; Cold flame. Same reach/height/cast/hot as `torch` — only the colour differs,
                ; so any difference seen in-world is attributable to colour alone.
                0.55 &thing.light.r set
                0.75 &thing.light.g set
                1.0 &thing.light.b set
                1.0 &thing.light.intensity set
                16.0 &thing.light.reach set
                0.35 &thing.light.radius set
                2.5 &thing.light.height set
                1 &thing.light.cast set
                0 &thing.light.hot set
                1 &thing.light.flicker set
                0 return
            @on_destroy>
                &thing.destroy call drop
                0 return
