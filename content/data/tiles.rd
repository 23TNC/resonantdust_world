<tile>
    ::grass>
        :data>
            @define>
                ; texture-generalization P0: every tile participates in lighting via its
                ; def — ground receives LIKE GROUND (mode 2), casts nothing.
                0 &tile.cast_shadow set
                2 &tile.receives_shadows set
                0 return
            @on_create>
                0 return
    ::dirt>
        :data>
            @define>
                ; texture-generalization P0: every tile participates in lighting via its
                ; def — ground receives LIKE GROUND (mode 2), casts nothing.
                0 &tile.cast_shadow set
                2 &tile.receives_shadows set
                0 return
            @on_create>
                0 return
    ::sand>
        :data>
            @define>
                ; texture-generalization P0: every tile participates in lighting via its
                ; def — ground receives LIKE GROUND (mode 2), casts nothing.
                0 &tile.cast_shadow set
                2 &tile.receives_shadows set
                0 return
            @on_create>
                0 return
    ::water>
        :data>
            @define>
                ; texture-generalization P0: every tile participates in lighting via its
                ; def — ground receives LIKE GROUND (mode 2), casts nothing.
                0 &tile.cast_shadow set
                2 &tile.receives_shadows set
                0 return
            @on_create>
                0 return
    ::stone>
        :data>
            @define>
                ; texture-generalization P0: every tile participates in lighting via its
                ; def — ground receives LIKE GROUND (mode 2), casts nothing.
                0 &tile.cast_shadow set
                2 &tile.receives_shadows set
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
                1 &tile.height set
                ; texture-generalization P0: the LINKED atlas geometry lives in content
                ; (boot-available — never the async texture manifest): a 4x4 autotile
                ; sheet with 1 unit of INTERNAL between-cell padding (distinct from the
                ; manifest's external pad, which is 0). rotation is the per-type MODE:
                ; 1 = AUTOTILE (cell from same-rule cardinal neighbors). Walls receive
                ; LIKE A BILLBOARD (mode 1) and cast NOTHING yet (wall shadows later).
                4 &tile.linked.w set
                4 &tile.linked.h set
                1 &tile.padding set
                1 &tile.rotation set
                0 &tile.cast_shadow set
                1 &tile.receives_shadows set
                0 return
            @on_create>
                0 return
