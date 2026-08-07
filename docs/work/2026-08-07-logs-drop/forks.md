# Forks — logs-drop (plan-time decisions; each is mine unless the user vetoes)

## F1 — `yields` lives on the carrier binding {#f1}

**Chosen**: `interactions = [{ name = "cut_down", yields = "logs" }]` — authored per
CARRIER. The tree yields logs; shrub and cactus bind cut_down with no yield and fell to
nothing, exactly as they do today. The binding is already the per-carrier lane
(`magnitude`); the loader validates `yields` names a known thing kind at LOAD.
**Rejected**: `yields` on the `[[interaction]]` (every fellable would drop logs — wrong
for shrubs, and un-authorable per species); a separate `fell_tree` interaction per yield
(defs multiply for what one binding field expresses).

## F2 — the yield rides the destroy SET as a REPLACE {#f2}

**Chosen**: when the felled carrier's binding authors `yields`, the composer's ONE
clearing SET swaps its `kind_reference` operand from `0` to the yielded thing's
`kind_reference` (variant 0). One event, one cell, no clear-then-place pair to race, and
the client's kind-carrying override path (proven by torches/walls) draws it.
**Rejected**: a second SET after the tombstone (two writes to one cell in one program —
ordering becomes load-bearing for nothing); a CREATE-minted entity (logs are cold
scatter, not a pawn — the thing layer is their home until hauling exists).

## F3 — logs are placeholder-visual scatter with no interactions {#f3}

A new `[[thing]] logs` (`type = "biome-thing"`, `kind = "logs"`): white texture +
log-brown tint at sub-tile scale — the shrub/rock placeholder pattern, no art-pipeline
work. NO `interactions` binding: logs are not tree-like (lumberjack F6 stands), and
picking them up needs the hauling/inventory successor, which this stream only records.
Real log art is one manifest + master drop-in later; nothing here blocks it.
