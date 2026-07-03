; Thing visuals — a tinted `thing` prim per flora type. Cold-zone things aren't
; drawn client-side yet (only tiles and object-shard free things are), so these
; are authored ahead of that renderer landing, in the same prim+tint shape tiles
; use.
<thing>
    ::tree>
        :visual>
            @on_create>
                "thing ^prim call &thing export
                ; The conifer master in R2 (master/world.0/conifer.0/1.1.0.albedo.png);
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
                ; (master/linked.0/wall_smooth.0/1.l.0.1.albedo.png) and derives the
                ; preview. Uses wall_smooth because it's mastered (rock_smooth is
                ; still a raw sprite — `bin/art remaster` masters new objects). tint
                ; stays white so it doesn't recolour the art; geoColor is the flat
                ; silhouette shown until the sprite streams in.
                "linked/wall_smooth &thing.texture set
                #ffffff &thing.tint set
                #7a7a7a &thing.geoColor set
                0 return
            @on_destroy>
                0 return
