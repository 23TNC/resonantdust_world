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
                "biome-thing/default/conifer/default &thing.texture set
                #ffffff &thing.tint set
                #2f4a2a &thing.geoColor set
                ; size is the SQUARE sprite scale in TILES (default 1). 2 → the conifer
                ; draws 2 tiles tall on its 1×1 footprint, so it rises PAST its cell and
                ; we can see how overlapping things z-order (anchor row, bottom-of-screen
                ; wins). anchor + sprite_anchor y=1 pin the trunk base to the cell's front
                ; edge (bottom-centre), so the extra height grows UP. x stays centred (0.5).
                2 &thing.size set
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
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                0 return
            @on_destroy>
                0 return
    ::rock>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                ; A real texture stem: the client appends a direction (`l` for a
                ; linked-category object) and the gate resolves it to a master albedo
                ; (master/linked/wall.smooth/1.l.0.1.albedo.png) and derives the
                ; preview. Uses wall.smooth because it's mastered (rock.smooth is
                ; still a raw sprite — `bin/art remaster` masters new kinds). tint
                ; stays white so it doesn't recolour the art; geoColor is the flat
                ; silhouette shown until the sprite streams in.
                "linked/wall.smooth &thing.texture set
                #ffffff &thing.tint set
                #7a7a7a &thing.geoColor set
                0.5 &thing.size set
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
                "biome-thing/default/flora/default &thing.texture set
                #ffffff &thing.tint set
                #4a7a3a &thing.geoColor set
                0.5 &thing.size set
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
                "pawn/animal/wolf/default &thing.texture set
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
                1.0 &thing.anchor.y set
                1.0 &thing.sprite_anchor.y set
                0 return
            @on_destroy>
                0 return
