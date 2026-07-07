; Thing visuals — a tinted `thing` prim per flora type. Cold-zone things aren't
; drawn client-side yet (only tiles and object-shard free things are), so these
; are authored ahead of that renderer landing, in the same prim+tint shape tiles
; use.
<thing>
    ::tree>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                ; The conifer master (master/world/conifer/1.s.0.1.albedo.png);
                ; the gate resolves the "world/conifer stem and derives the preview.
                ; tint stays white so it doesn't recolour the art; geoColor is the
                ; flat green silhouette shown until the sprite streams in.
                "world/conifer &thing.texture set
                #ffffff &thing.tint set
                #2f4a2a &thing.geoColor set
                ; size is the DRIVING (min) axis in PIXELS. 64 = one tile wide; the
                ; conifer's 1:2 texture then makes it 2 tiles tall, so it draws PAST
                ; its cell and we can see how overlapping things z-order (the host
                ; bottom-anchors + sorts by base row). Other flora leave size unset →
                ; the host's small default.
                64 &thing.size set
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
                0 return
            @on_destroy>
                0 return
    ::cactus>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "white &thing.texture set
                #3e6b3a &thing.tint set
                0 return
            @on_destroy>
                0 return
    ::reed>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                "white &thing.texture set
                #6f7d3a &thing.tint set
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
                0 return
            @on_destroy>
                0 return
