<tile>
    ::grass>
        :visual>
            @on_create>
                "tile ^prim call &tile export
                "white &tile.texture set
                #4b573e &tile.tint set
                0 return
            @on_destroy>
                &tile.destroy call drop
                0 return
    ::dirt>
        :visual>
            @on_create>
                "tile ^prim call &tile export
                "white &tile.texture set
                #653d00 &tile.tint set
                0 return
            @on_destroy>
                0 return
    ::sand>
        :visual>
            @on_create>
                "tile ^prim call &tile export
                "white &tile.texture set
                #c2b280 &tile.tint set
                0 return
            @on_destroy>
                0 return
    ::water>
        :visual>
            @on_create>
                "tile ^prim call &tile export
                "white &tile.texture set
                #2e5a78 &tile.tint set
                0 return
            @on_destroy>
                0 return
    ::stone>
        :visual>
            @on_create>
                "tile ^prim call &tile export
                "white &tile.texture set
                #6b6b6b &tile.tint set
                0 return
            @on_destroy>
                0 return
    ; The smooth wall's visual — the LINKED 4x4 atlas (16 neighbor orientations,
    ; build-walls D1: cell x = N + 2E, y = 3 - (S + 2W)); the client picks the cell
    ; from same-kind neighbors at expansion. White tint keeps the art's own colours;
    ; the gray geo fill shows until the atlas streams in. The BLUEPRINT stem derives
    ; by convention (material segment -> "blueprint"), not authored per kind (F1).
    ::wall_smooth>
        :visual>
            @on_create>
                "tile ^prim call &tile export
                "biome-tile/default/smooth/wall &tile.texture set
                #ffffff &tile.tint set
                0 return
            @on_destroy>
                &tile.destroy call drop
                0 return
