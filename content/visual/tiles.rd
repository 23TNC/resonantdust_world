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
        