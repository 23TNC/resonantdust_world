# Blockers — unified data texture + scatter

_Open blockers; remove the row when cleared._

---

- **P4 close · command-format v2 semantics (USER).** The user proposed evolving the command format
  mid-P4: 16 u16-addressable bands ("sets") leaned into hard — u8 command OPCODE (future: presence/
  other-map writes), u6 counts, 64×64 buffers allocated per table (globals / defs / prims / lights
  + 12 free). Two ambiguities need the author before the live format is re-cut: (1) the 16× u6
  header fields — per-band counts? ("one u16 block for each byte in our command header" doesn't
  quite parse against u6s); (2) "allocate one …" — one BUFFER per table or one header LANE per
  table in a shared buffer. The live 5-px-group format is verified + committed; v2 is a refinement
  pass on top. User is also live-verifying orbit perf right now.
