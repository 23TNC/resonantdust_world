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
