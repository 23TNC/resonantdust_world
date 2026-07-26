; Primary-layer things (thing.1) — flora a biome scatters over its ground tiles.
; Their own def_id namespace (object_id), independent of tiles. A biome's
; @on_create names one of these; worldgen resolves the name to an object_id and
; packs it into the zone's thing entries.
<thing>
    ::tree>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::shrub>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::cactus>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::reed>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ::rock>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ; Appended last so existing thing def_ids don't renumber (append-compatible).
    ::flora>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ; The wolf — the world's first PAWN. Unlike the flora above it is not a
    ; scattered cold thing: it's a mobile shard entity spawned + wandered by the
    ; npc bot. This def exists only to give the client its visual by kind
    ; (object_id); worldgen never references `wolf`, so it's never seeded as terrain.
    ; Kept last so its object_id stays a stable append (see KIND_WOLF in the npc).
    ::wolf>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ; The torch — the world's first LIGHT-EMITTING kind. Appended after `wolf` so no
    ; existing object_id renumbers (`wolf`'s must not move — the npc pins KIND_WOLF).
    ; Scattered sparsely by worldgen; the light itself is authored on the VISUAL side
    ; (`&thing.light.*`), because emission is a presentation, not simulation state.
    ::torch>
        :data>
            @define>
                0 return
            @on_create>
                0 return
