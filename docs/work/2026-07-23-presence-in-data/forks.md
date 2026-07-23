# Forks — presence + buckets into the data texture

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Tie eviction to the zone subscription model {#f1}

**2026-07-23 — user CONSIDERING; build the plain region-owner eviction first, layer this after.**

Instead of computing region-boundary crossings independently, hang the data-texture materialisation
off the subscription lifecycle. Clean two-layer split:

- **Subscription = CPU residency + hysteresis authority** (existing cost-based release + hot-hold +
  512-LRU). That 512 > 256 (a region) is the **feature**: the LRU is a **materialisation cache** —
  zones stay CPU-resident past the region edge, so crossing a 16-zone boundary is a **network-free
  re-scatter**, not a re-fetch.
- **Data texture = the anchor-centred region materialisation** of that set. Zone enters region →
  scatter; leaves → clear. Lifecycle: `subscribe → (enters region) scatter → (leaves region) clear →
  (leaves 512-LRU) drop CPU data`.

**Safety already proven** ([`README`](README.md#eviction)): prim/light eviction rides the DEEP
(subscription) layer, presence/buckets the SHALLOW (region-window) layer; reach (<1 zone) fits
inside the gap, so no reach-lag machinery. **Deferred** because the plain region-owner eviction (P2)
is self-contained + verifiable; this is a refinement that also touches the subscription refactor.

## F2 · Self-address (7 slots) vs 2-px command (8 slots) {#f2}

**2026-07-23 — CHOSEN: 7 slots + self-address.**

Presence is a full 128-bit texel — no free bits for the u16 id. Two ways to command it:
- **(chosen) spend 1 of 8 slots on the id** → 7 lights / 7 casters per tile, but presence rides the
  identical 1px pure-payload path as defs/prims/lights (no 2px form, no opcode-1 branch in the
  scatter shader).
- **keep 8 slots + a 2px command** (px0 position_reference, px1 payload) → costs a px per presence
  write AND a position-decode/pmod path in the scatter vertex shader.

7 slots wins: >7 lights (or casters) overlapping ONE tile is rare, and command-path uniformity is
worth more than the 8th slot. Revisit only if per-tile overlap saturates.

## F3 · Beyond-region-zoom artifact {#f3}

**2026-07-23 — ACCEPTED degradation; optional guard noted.**

Past one region of zoom-out, a LOSER tile (whose slot is owned by a winner 256 tiles away) reads the
winner's presence/bucket → a faint wrong shadow rather than nothing. At ~4 px/tile that's noise, and
the user explicitly allows best-effort beyond-region zoom. If it ever shows: store a few high bits of
the owner's world-zone in the slot, compare on read, mismatch → treat as empty. NOT worth the bits
speculatively.

## F4 · Prim/light defrag {#f4}

**2026-07-23 — DEFERRED (free-list is the must-have).**

The free-list bounds the high-water mark to peak concurrent objects (≪ 65 536) and is correct on its
own — an id is reused only after zone eviction proves it unreferenced. Defrag (compact live records
to a dense low range) is optional and only for iteration/upload density, never correctness — the set
is a fixed 65 536-texel texture, so sparsity costs nothing but density. Build if/when a dense active
range is actually needed.
