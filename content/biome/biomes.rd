; Biomes — the DSL that classifies one tile of terrain.
;
; Worldgen runs this per cell, independently: it samples N biome DIMENSIONS from
; noise at the cell's world position, then walks the biomes in file order and
; takes the FIRST whose `@define` returns non-zero. That biome's `@on_create`
; decides the ground tile and any primary-layer thing.
;
; Surface the host provides:
;   ^biome call   — pushes the sampled dimensions as an array. Grab them with
;                   `^biome call &biome set`, then read a dimension with
;                   `*biome.0` / `*biome.1` / … (the operand stack is per-line, so
;                   stash into a slot on one line and test it on the next).
;   <salt> ^rand call — a deterministic [0,1) draw for this tile, salted by the
;                   int. Use a distinct salt per feature so trees and flora
;                   scatter without correlating.
;
; Dimensions (all in [0,1], sampled from independent noise fields):
;   biome.0 = temperature
;   biome.1 = humidity
;   biome.2 = elevation   (below ~0.35 is under water)
;
; Output slots @on_create writes:
;   &tile set     — the ground tile name (floor / wall / cliff): grass, dirt,
;                   sand, water, stone, …
;   &thing.1 set  — the primary thing-layer name (a tree, a shrub); omit to leave
;                   the cell bare. Only one thing layer exists today (thing.1); a
;                   second `&thing.1 set` on a later line overwrites the first, so
;                   order the scatter draws least- to most-dominant.
;
; Order = priority. Elevation bands (ocean/beach/mountain) are tested first, then
; climate biomes, then the plains catch-all last (its define is always true).
;
; @subtype>  — each biome's STABLE `subtype_id` (docs/object-model.md): a
;   biome-tile/biome-thing object carries its biome in `subtype`, so this is the
;   biome's identity in a stored zone. Authored EXPLICITLY (a constant) so it is
;   decoupled from THIS file's order, which is evaluation priority and may be
;   retuned — the id must never renumber (stored zones carry it). Append a new
;   biome with the next free id; reorder freely for priority. `0` is reserved
;   (the `default` / biome-agnostic subtype).
<biome>
    ::ocean>
        @subtype>
            1 return
        @define>
            ^biome call &biome set
            *biome.2 0.32 lt return
        @on_create>
            water &tile set
            0 return
    ::beach>
        @subtype>
            2 return
        @define>
            ^biome call &biome set
            *biome.2 0.38 lt return
        @on_create>
            sand &tile set
            1 ^rand call 0.03 lt if rock &thing.1 set
            0 return
    ::mountains>
        @subtype>
            3 return
        @define>
            ^biome call &biome set
            *biome.2 0.82 ge return
        @on_create>
            stone &tile set
            2 ^rand call 0.08 lt if rock &thing.1 set
            0 return
    ::wetland>
        @subtype>
            4 return
        @define>
            ^biome call &biome set
            *biome.1 0.60 ge *biome.2 0.46 lt and return
        @on_create>
            dirt &tile set
            3 ^rand call 0.25 lt if reed &thing.1 set
            0 return
    ::desert>
        @subtype>
            5 return
        @define>
            ^biome call &biome set
            *biome.0 0.68 ge *biome.1 0.32 lt and return
        @on_create>
            sand &tile set
            4 ^rand call 0.04 lt if cactus &thing.1 set
            0 return
    ::forest>
        @subtype>
            6 return
        @define>
            ^biome call &biome set
            *biome.1 0.55 ge *biome.0 0.35 ge and *biome.0 0.80 le and return
        @on_create>
            grass &tile set
            8 ^rand call 0.16 lt if flora &thing.1 set
            5 ^rand call 0.14 lt if shrub &thing.1 set
            6 ^rand call 0.22 lt if tree &thing.1 set
            0 return
    ::plains>
        @subtype>
            7 return
        @define>
            1 return
        @on_create>
            grass &tile set
            9 ^rand call 0.20 lt if flora &thing.1 set
            7 ^rand call 0.06 lt if shrub &thing.1 set
            0 return
