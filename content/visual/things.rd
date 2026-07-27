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
                ; MATERIAL variation on the packed map: the conifer's albedo splits into
                ; ch0 = foliage (#b7cf5d) + ch1 = trunk (#b0754f) (see `art split_layers
                ; world/conifer`). The canonical reconstruction is
                ;   out = packed_residual + packed.R*jitter(tint0) + packed.G*jitter(tint1)
                ; so BOTH channels must set their base-colour tint (else that region's
                ; colour, subtracted into the residual, is lost). ch0 (foliage) also binds
                ; the `strand` material so the needles gain fine hue/chroma variation —
                ; colour, not light; ch1 (trunk) is tint-only (no jitter).
                "strand &thing.packed.0.material set
                #46d64f &thing.packed.0.tint set
                ; ch1 (trunk): NO material, just its natural base colour so the residual
                ; reconstructs the original brown trunk faithfully (un-restyled).
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
                ; Warm flame colour. `reach` is in TILES, and it is THE cost dial for a MOVING light —
                ; measured 2026-07-26 at zoom 1 with 3 orbiting torches: reach 16 → 18 fps, 12 → 30 fps,
                ; 8 → 120 fps (work `2026-07-26-moving-lights` I2). It compounds three ways: the shadow
                ; walk runs from a texel to its light so walk length ∝ reach; the texels a light claims
                ; go as reach²; and more reach means more lights overlap each texel, multiplying the
                ; walks per texel. Static lights are unaffected — they bake once — so this is a budget
                ; on MOTION, not on light count.
                ;
                ; RAISED 8 → 20 (2026-07-27) to make long-range shadows inspectable: penumbra WIDTH is
                ; independent of horizontal distance (it is D·h/(Lz−h), no distance term) while shadow
                ; LENGTH grows linearly with it, so judging the soft edge needs shadows that run a long way.
                ; Affordable because `hot 0` below makes these STATIC: they bake once, and the reach→fps
                ; figures above are a budget on MOTION. Revisit if a torch is ever authored `hot 1`.
                ; The wash concern the 8 was chosen for still stands for looks: 20 claims 40 × 40 tiles
                ; against a 28 × 12 visible area, so this is a LEGIBILITY setting, not a final art call.
                ; `cast 1` = occludes (it participates in the shadow walk); `hot 0` = STATIC,
                ; so it bakes once and costs nothing per frame — the property that makes
                ; many torches affordable.
                1.0 &thing.light.r set
                0.85 &thing.light.g set
                0.55 &thing.light.b set
                1.0 &thing.light.intensity set
                20.0 &thing.light.reach set
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
                20.0 &thing.light.reach set
                0.35 &thing.light.radius set
                2.5 &thing.light.height set
                1 &thing.light.cast set
                0 &thing.light.hot set
                0 return
            @on_destroy>
                &thing.destroy call drop
                0 return
