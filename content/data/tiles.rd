<tile>
    ::grass>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::dirt>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::sand>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::water>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::stone>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ; The smooth wall — the FIRST buildable kind (build-walls stream). The `tile.build`
    ; lane names the BUILD CATEGORY ("wall") — the build menu populates from tiles
    ; carrying it (D3: content alone lights the icon; a brick wall is one more entry
    ; here). Appended so no existing def_id renumbers (ids are positional).
    ::wall_smooth>
        :data>
            @define>
                "wall &tile.build set
                ; tile-lighting F2: a tile with height > 0 PARTICIPATES in the cold
                ; lighting class (receiver N-L via its atlas normal + a caster card of
                ; the art's box). Floors author nothing and stay pure ground.
                1 &tile.height set
                0 return
            @on_create>
                0 return
