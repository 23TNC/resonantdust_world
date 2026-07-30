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
    ; Appended so its object_id stays stable; consumers resolve it from THIS corpus
    ; by name (`thing_object_id("wolf")` — the npc fetches `/content`), never a pinned constant.
    ::wolf>
        :data>
            @define>
                ; speed: TICS PER TILE (pawn-movement F1) — 12 → 2 s/tile at 6 Hz.
                12 &thing.speed set
                0 return
            @on_create>
                0 return
    ; The torch — the world's first LIGHT-EMITTING kind. Appended after `wolf` so no
    ; existing object_id renumbers (ids are positional; stability is the append rule).
    ; Scattered sparsely by worldgen; the light itself is authored on the VISUAL side
    ; (`&thing.light.*`), because emission is a presentation, not simulation state.
    ::torch>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ; The blue torch. Data-identical to `torch` — the colour lives entirely on the visual
    ; side, so simulation sees no difference between them. Appended last so no object_id moves.
    ::torch_blue>
        :data>
            @define>
                0 return
            @on_create>
                0 return
    ; The humans — the first MULTI-PART pawns (human-pawns): body (part 0) + head (part 1),
    ; declared in content/visual/pawns.rd; which body/head each pawn wears rides its payload
    ; (`PART(slot, def)` — TABLES.md §payload). Two kinds because the sexes have separate
    ; sprite sets (pawn/human/female, pawn/human/male). Appended so no object_id renumbers;
    ; consumers resolve by name from THIS corpus, never a pinned constant.
    ::human_female>
        :data>
            @define>
                ; speed: TICS PER TILE — 16 → ~2.7 s/tile at 6 Hz (slower than the wolf's 12).
                16 &thing.speed set
                0 return
            @on_create>
                0 return
    ::human_male>
        :data>
            @define>
                16 &thing.speed set
                0 return
            @on_create>
                0 return
