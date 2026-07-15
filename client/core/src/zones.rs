//! Anchor-driven zone subscription manager.
//!
//! The client holds a list of **anchors** — points of interest in the world —
//! and from them decides which zones to keep a live [`SubZone`] open on. The list
//! is empty by default; a host adds anchors and moves them. The pixijs client
//! will add one anchor per viewport and slide it as the view pans; later, pawns
//! (souls) will carry their own anchors (the `soul` field reserves that path,
//! ported from the old game — pawn→anchor derivation is not built here yet).
//!
//! ## Tiers — a hysteresis ladder
//!
//! Each anchor projects four nested tile-radii (active ⊆ hot ⊆ warm ⊆ cold). A
//! zone takes the **tightest** tier any anchor assigns it (max-tier-wins). Unlike
//! the old game — whose tiers mapped to *partial* subscriptions (a card stream vs.
//! a tile stream) — the new world's [`SubZone`](crate::protocol::ClientMsg::SubZone)
//! pulls a zone's whole dataset in one frame, so a tier here means *how strongly
//! we hold that one subscription*:
//!
//!   - **active** — *enter*. A zone entering any anchor's active disk opens a sub.
//!     This is the only tier that opens a sub, so size a viewport's active radius
//!     to cover the whole visible area (+ a prefetch margin).
//!   - **hot** — *hold* (hysteresis). An open sub stays open, hard-held, while in
//!     hot. A zone reached only at hot is **not** opened — hot sustains, never
//!     starts. This is the anti-thrash band as the view pans across an edge.
//!   - **warm / cold / uncovered** — *soft-held*. Once a sub falls out of hot it is
//!     **not dropped by distance**. We keep the subscription and let the cost-based
//!     release (below) decide, because re-transmitting a zone the player pans back
//!     to is precisely the waste we're avoiding.
//!
//! Moving outward a sub walks active → hot → soft-held; it only ever *opens* by
//! crossing into active, and only ever *closes* by the soft release or the LRU.
//!
//! ## Soft release — retention vs. re-transmit
//!
//! A subscription is cheap to hold while its zone is quiet and dear to re-open when
//! the player returns, so we hold generously and release only once holding has
//! actually cost something. Per sub we track two byte tallies:
//!
//!   - **`load_bytes`** — the baseline burst streamed within [`LOAD_SETTLE_MS`] of
//!     the sub opening (its tiles + things + state). Our estimate of what a fresh
//!     `sub_zone` on this zone would re-transmit.
//!   - **`held_bytes`** — bytes streamed to the zone *since it went soft-held* — the
//!     deltas we pay for while the player is elsewhere.
//!
//! A soft-held sub releases the instant `held_bytes >= load_bytes`: retention has
//! cost as much as a re-fetch would, so nothing is saved by holding it longer. A
//! **quiet** zone (no deltas) never reaches that and stays subscribed indefinitely —
//! exactly the static tiles/things we never want to re-transmit — until the
//! capacity-LRU reclaims it. A **churning** zone pays a re-fetch's worth quickly and
//! is let go.
//!
//! ## Capacity-LRU
//!
//! A hard ceiling ([`DEFAULT_MAX_OPEN_SUBS`]) backstops the soft-hold: over it, the
//! least-recently-demoted soft-held sub goes first
//! ([`ZoneManager::enforce_capacity`]); a hard-held (active/hot) sub is never
//! evicted, so the cap can be exceeded when every open sub is hard-held.
//!
//! Sans-IO: decisions become [`ZoneIntent`]s the engine drains and maps to
//! `sub_zone` / `unsub` frames. `now` (ms) and the row's byte length are threaded in
//! for the load-settle window and LRU recency, so the manager stays pure and
//! unit-testable.

use std::collections::{HashMap, HashSet};

use resonantdust_codec::packed::{pack_zone_id, REALM_DIM, REGION_DIM, ZONE_DIM};

/// Default ceiling on simultaneously open zone subscriptions. Candidates beyond
/// it are evicted least-recently-demoted first; hard-held subs are never evicted,
/// so the cap can be exceeded when every open sub is hard-held.
pub const DEFAULT_MAX_OPEN_SUBS: usize = 512;

/// Per-tier tile radii an anchor projects (active ⊆ hot ⊆ warm ⊆ cold expected).
/// `0` (or negative) = the anchor does not provide that tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnchorRadii {
    pub active: i32,
    pub hot: i32,
    pub warm: i32,
    pub cold: i32,
}

impl AnchorRadii {
    /// Tiers paired with their radius, tightest first.
    fn tiers(&self) -> [(ZoneTier, i32); 4] {
        [
            (ZoneTier::Active, self.active),
            (ZoneTier::Hot, self.hot),
            (ZoneTier::Warm, self.warm),
            (ZoneTier::Cold, self.cold),
        ]
    }
}

/// The band a zone falls in for its closest-binding anchor (active tightest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneTier {
    Active,
    Hot,
    Warm,
    Cold,
}

fn tier_rank(t: ZoneTier) -> u8 {
    match t {
        ZoneTier::Active => 3,
        ZoneTier::Hot => 2,
        ZoneTier::Warm => 1,
        ZoneTier::Cold => 0,
    }
}

/// A subscription decision the manager made that needs IO. The engine drains
/// these (via [`ZoneManager::take_intents`]) and maps each to a `sub_zone`
/// (`on = true`) or `unsub` (`on = false`) frame, owning the `sid` ↔ `zone_id`
/// mapping itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneIntent {
    pub zone_id: u32,
    pub on: bool,
}

/// An anchor: a world point projecting subscription tiers around itself. Keyed by
/// a host-chosen name in the manager, so `"viewport:0"` (or `"soul:<id>"`) updates
/// in place as it moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Anchor {
    /// Global tile coordinates of the anchor's centre.
    tile_x: i32,
    tile_y: i32,
    radii: AnchorRadii,
    /// The soul (pawn card id) this anchor represents, or `0` for a non-soul
    /// anchor such as a viewport. Reserved for the future per-soul memory path;
    /// unused by subscription logic today.
    soul: u32,
}

/// How long after a sub opens its inbound bytes count as the **initial load** (the
/// re-transmit estimate) rather than retention. The baseline burst (tiles + things +
/// state) lands within a round-trip of the `sub_zone`; this is comfortably longer so
/// a dense zone's whole baseline is captured, yet anchored at open time so it only
/// ever covers that first burst — later deltas, and every re-visit, fall past it.
const LOAD_SETTLE_MS: u64 = 1_000;

/// Live state for one open zone subscription. A `Sub` exists in the map iff we
/// believe a `SubZone` is open for it; closing removes the entry.
#[derive(Debug, Clone, Copy)]
struct Sub {
    /// The tier last assigned while open. `Active`/`Hot` are hard-held; anything
    /// looser is soft-held (see `candidate_since_ms`).
    tier: ZoneTier,
    /// `Some(ms)` once the sub went soft-held, reset to `None` whenever it re-hardens
    /// (back to active/hot). Orders the capacity-LRU (least-recently-demoted first).
    candidate_since_ms: Option<u64>,
    /// When the `SubZone` opened (ms). Bytes within [`LOAD_SETTLE_MS`] of it seed
    /// `load_bytes`; later ones are deltas.
    opened_ms: u64,
    /// Estimated re-transmit cost: bytes streamed during the initial-load window —
    /// what a fresh `sub_zone` on this zone would re-send.
    load_bytes: u64,
    /// Bytes streamed to the zone since it last went soft-held — the retention we pay
    /// while the player is elsewhere. Reset on each (re-)entry to soft-held; once it
    /// reaches `load_bytes` the sub is released.
    held_bytes: u64,
}

/// The anchor-driven zone subscription manager. Sans-IO: mutate it with
/// [`set_anchor`](Self::set_anchor) / [`remove_anchor`](Self::remove_anchor) /
/// [`note_update`](Self::note_update), then drain [`take_intents`](Self::take_intents).
pub struct ZoneManager {
    anchors: HashMap<String, Anchor>,
    /// Open subscriptions, keyed by `zone_id`. Presence == open.
    subs: HashMap<u32, Sub>,
    intents: Vec<ZoneIntent>,
    max_open_subs: usize,
}

impl Default for ZoneManager {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_OPEN_SUBS)
    }
}

impl ZoneManager {
    pub fn new(max_open_subs: usize) -> Self {
        Self {
            anchors: HashMap::new(),
            subs: HashMap::new(),
            intents: Vec::new(),
            max_open_subs: max_open_subs.max(1),
        }
    }

    /// Add or move the anchor named `name`. Idempotent: an unchanged anchor is a
    /// no-op (no recompute, no intents) — cheap to call every pan frame.
    pub fn set_anchor(
        &mut self,
        name: &str,
        tile_x: i32,
        tile_y: i32,
        radii: AnchorRadii,
        soul: u32,
        now: u64,
    ) {
        let anchor = Anchor {
            tile_x,
            tile_y,
            radii,
            soul,
        };
        if self.anchors.get(name) == Some(&anchor) {
            return;
        }
        self.anchors.insert(name.to_string(), anchor);
        self.recompute(now);
    }

    /// Remove the anchor named `name`, recomputing subscriptions if it existed.
    pub fn remove_anchor(&mut self, name: &str, now: u64) {
        if self.anchors.remove(name).is_some() {
            self.recompute(now);
        }
    }

    /// Record an inbound row of `bytes` for `zone_id`. Within a sub's initial-load
    /// window the bytes seed its re-transmit estimate; once the sub is soft-held they
    /// accrue as retention, and when retention reaches that estimate the sub is
    /// released (holding it has now cost as much as re-fetching would). A hard-held sub
    /// past its load window just renders the delta — no cost is tracked, no eviction.
    pub fn note_update(&mut self, zone_id: u32, bytes: u64, now: u64) {
        let release = match self.subs.get_mut(&zone_id) {
            // Initial-load window: (re-)transmit cost, whether or not the player has
            // already panned past — so a fast pan-by never instant-drops a zone the
            // moment its baseline lands.
            Some(s) if now.saturating_sub(s.opened_ms) < LOAD_SETTLE_MS => {
                s.load_bytes = s.load_bytes.saturating_add(bytes);
                false
            }
            // Soft-held past its load window: charge retention, release once it has
            // cost a whole re-fetch.
            Some(s) if s.candidate_since_ms.is_some() => {
                s.held_bytes = s.held_bytes.saturating_add(bytes);
                s.held_bytes >= s.load_bytes
            }
            _ => false,
        };
        if release {
            self.close_sub(zone_id);
        }
    }

    /// Drain the pending subscription intents. The engine maps them to frames.
    pub fn take_intents(&mut self) -> Vec<ZoneIntent> {
        std::mem::take(&mut self.intents)
    }

    /// Drop every anchor and sub (no close intents are emitted — used on
    /// disconnect, where the socket and its subscriptions are already gone).
    pub fn clear(&mut self) {
        self.anchors.clear();
        self.subs.clear();
        self.intents.clear();
    }

    /// Number of currently-open subscriptions.
    pub fn open_sub_count(&self) -> usize {
        self.subs.len()
    }

    // ── internals ────────────────────────────────────────────────────────────

    /// The tightest tier each covered zone wants, max-tier-wins across all anchors.
    fn desired_tiers(&self) -> HashMap<u32, ZoneTier> {
        let mut desired: HashMap<u32, ZoneTier> = HashMap::new();
        for anchor in self.anchors.values() {
            anchor_coverage(anchor, &mut |zone, tier| {
                let e = desired.entry(zone).or_insert(tier);
                if tier_rank(tier) > tier_rank(*e) {
                    *e = tier;
                }
            });
        }
        desired
    }

    /// Reconcile open subs against the anchors' desired tiers (the hysteresis
    /// ladder), then enforce the capacity ceiling.
    fn recompute(&mut self, now: u64) {
        let desired = self.desired_tiers();

        // Visit every zone that is either wanted or currently open, so departed
        // zones get closed and re-entered ones re-harden.
        let mut zones: HashSet<u32> = desired.keys().copied().collect();
        zones.extend(self.subs.keys().copied());

        for zone in zones {
            let tier = desired.get(&zone).copied();
            let open = self.subs.contains_key(&zone);
            match tier {
                // Enter / hard-hold.
                Some(ZoneTier::Active) => {
                    if !open {
                        self.open_sub(zone, now);
                    }
                    self.harden(zone, ZoneTier::Active);
                }
                // Hysteresis hold: sustain an open sub hard-held; never open fresh.
                Some(ZoneTier::Hot) => {
                    if open {
                        self.harden(zone, ZoneTier::Hot);
                    }
                }
                // Anything looser than hot — warm, cold, or fully uncovered — is
                // soft-held: keep the sub and let the cost-based release
                // (`note_update`) or the capacity-LRU decide. Distance alone never
                // drops a sub now; it only ever *opened* by crossing into active.
                Some(t) => {
                    if open {
                        self.make_soft(zone, t, now);
                    }
                }
                None => {
                    if open {
                        self.make_soft(zone, ZoneTier::Cold, now);
                    }
                }
            }
        }

        self.enforce_capacity();
    }

    /// Open a sub for `zone` at `now` (inserted hard-held at active; the caller
    /// assigns the final tier) and emit the intent.
    fn open_sub(&mut self, zone: u32, now: u64) {
        self.subs.insert(
            zone,
            Sub {
                tier: ZoneTier::Active,
                candidate_since_ms: None,
                opened_ms: now,
                load_bytes: 0,
                held_bytes: 0,
            },
        );
        self.intents.push(ZoneIntent {
            zone_id: zone,
            on: true,
        });
    }

    /// Mark an open sub hard-held at `tier` (active/hot): clears any candidacy.
    fn harden(&mut self, zone: u32, tier: ZoneTier) {
        if let Some(s) = self.subs.get_mut(&zone) {
            s.tier = tier;
            s.candidate_since_ms = None;
        }
    }

    /// Soft-hold an open sub at `tier` (warm/cold). Stamps the demotion time and
    /// resets the retention budget only on the transition into soft-hold, so a sub
    /// that stays soft keeps its LRU age and does not forget the `held_bytes` it has
    /// paid since it first demoted.
    fn make_soft(&mut self, zone: u32, tier: ZoneTier, now: u64) {
        if let Some(s) = self.subs.get_mut(&zone) {
            s.tier = tier;
            if s.candidate_since_ms.is_none() {
                s.candidate_since_ms = Some(now);
                s.held_bytes = 0;
            }
        }
    }

    /// Close a sub and emit the intent.
    fn close_sub(&mut self, zone: u32) {
        if self.subs.remove(&zone).is_some() {
            self.intents.push(ZoneIntent {
                zone_id: zone,
                on: false,
            });
        }
    }

    /// Evict candidates until under the cap. The least-recently-demoted candidate
    /// goes first; hard-held subs are never evicted (so the cap may be exceeded
    /// when every open sub is hard-held).
    fn enforce_capacity(&mut self) {
        while self.subs.len() > self.max_open_subs {
            let victim = self
                .subs
                .iter()
                .filter_map(|(z, s)| s.candidate_since_ms.map(|t| (*z, t)))
                .min_by_key(|(_, t)| *t)
                .map(|(z, _)| z);
            match victim {
                Some(zone) => self.close_sub(zone),
                None => break, // all hard-held — nothing to reclaim
            }
        }
    }
}

/// Walk the zones an anchor covers, reporting each `(zone_id, tier)`. Each tier is
/// a Chebyshev tile-disk; the disk's tile span folds to the zones it overlaps.
/// Tiers with a non-positive radius and zones outside the world are skipped.
fn anchor_coverage(anchor: &Anchor, sink: &mut impl FnMut(u32, ZoneTier)) {
    for (tier, radius) in anchor.radii.tiers() {
        if radius <= 0 {
            continue;
        }
        let zx0 = zone_axis(anchor.tile_x - radius);
        let zx1 = zone_axis(anchor.tile_x + radius);
        let zy0 = zone_axis(anchor.tile_y - radius);
        let zy1 = zone_axis(anchor.tile_y + radius);
        for zgx in zx0..=zx1 {
            for zgy in zy0..=zy1 {
                if let Some(zone) = zone_at(zgx, zgy) {
                    sink(zone, tier);
                }
            }
        }
    }
}

/// Tiles per zone edge, as `i32` for coordinate math.
const ZONE_EDGE: i32 = ZONE_DIM as i32;
/// Zones per region edge.
const REGION_EDGE: i32 = REGION_DIM as i32;
/// Regions per realm edge.
const REALM_EDGE: i32 = REALM_DIM as i32;
/// Region cells per world axis (`realm_x`·`region_x` = 16×16 = 256 regions/axis).
const REGION_AXIS: i32 = REALM_EDGE * REGION_EDGE;
/// Zones per world axis — the valid range for a global zone coordinate.
const ZONES_PER_AXIS: i32 = REGION_AXIS * REGION_EDGE;

/// The global zone coordinate containing global tile coordinate `tile` (floor
/// division, so it is correct for off-origin tiles).
fn zone_axis(tile: i32) -> i32 {
    tile.div_euclid(ZONE_EDGE)
}

/// Pack a global zone coordinate `(zgx, zgy)` into a geographic `zone_id`, or `None` if it
/// falls outside the world. `zgx` nests realm ⊃ region ⊃ zone (each 16 per axis).
fn zone_at(zgx: i32, zgy: i32) -> Option<u32> {
    if !(0..ZONES_PER_AXIS).contains(&zgx) || !(0..ZONES_PER_AXIS).contains(&zgy) {
        return None;
    }
    let split = |zg: i32| -> (u8, u8, u8) {
        let zone = (zg % REGION_EDGE) as u8;
        let region_full = zg / REGION_EDGE;
        (
            (region_full / REALM_EDGE) as u8, // realm
            (region_full % REALM_EDGE) as u8, // region within realm
            zone,                             // zone within region
        )
    };
    let (realm_x, region_x, zone_x) = split(zgx);
    let (realm_y, region_y, zone_y) = split(zgy);
    Some(pack_zone_id(realm_x, realm_y, region_x, region_y, zone_x, zone_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Radii that put each ring in its own zone band around tile (8,8) (zone 0,0):
    /// active→zone(0,0), hot→±1 zone, warm→zones 2..3, cold→zone 4.
    fn ladder() -> AnchorRadii {
        AnchorRadii {
            active: 4,
            hot: 20,
            warm: 40,
            cold: 60,
        }
    }

    fn zone(zgx: i32, zgy: i32) -> u32 {
        zone_at(zgx, zgy).unwrap()
    }

    fn is_open(zm: &ZoneManager, z: u32) -> bool {
        zm.subs.contains_key(&z)
    }

    fn is_candidate(zm: &ZoneManager, z: u32) -> bool {
        zm.subs
            .get(&z)
            .map(|s| s.candidate_since_ms.is_some())
            .unwrap_or(false)
    }

    #[test]
    fn coverage_assigns_tightest_tier() {
        let mut zm = ZoneManager::default();
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        let desired = zm.desired_tiers();
        assert_eq!(desired.get(&zone(0, 0)), Some(&ZoneTier::Active));
        assert_eq!(desired.get(&zone(1, 0)), Some(&ZoneTier::Hot));
        assert_eq!(desired.get(&zone(2, 0)), Some(&ZoneTier::Warm));
        assert_eq!(desired.get(&zone(4, 0)), Some(&ZoneTier::Cold));
    }

    #[test]
    fn only_active_opens_a_sub() {
        let mut zm = ZoneManager::default();
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        // The active zone opens; the hot/warm/cold ring does not (hot+ only hold).
        assert!(is_open(&zm, zone(0, 0)));
        assert!(!is_open(&zm, zone(1, 0)));
        assert!(!is_open(&zm, zone(2, 0)));
        assert_eq!(zm.open_sub_count(), 1);
    }

    #[test]
    fn hysteresis_holds_then_soft_holds() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);

        // Open at active.
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        assert!(is_open(&zm, z) && !is_candidate(&zm, z));

        // Move so z is now only Hot: still open, still hard-held.
        zm.set_anchor("a", 28, 8, ladder(), 0, 10);
        assert!(is_open(&zm, z) && !is_candidate(&zm, z), "hot sustains hard");

        // Move so z is now Warm: still open, now soft-held.
        zm.set_anchor("a", 44, 8, ladder(), 0, 20);
        assert!(is_open(&zm, z) && is_candidate(&zm, z), "warm = soft-held");

        // Move so z is out of every band: still open — distance no longer drops it,
        // only the cost-based release or the capacity-LRU can.
        zm.set_anchor("a", 88, 8, ladder(), 0, 30);
        assert!(is_open(&zm, z) && is_candidate(&zm, z), "cold/uncovered soft-holds");
    }

    #[test]
    fn reentering_active_rehardens() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        zm.set_anchor("a", 44, 8, ladder(), 0, 10); // z → warm candidate
        assert!(is_candidate(&zm, z));
        zm.set_anchor("a", 8, 8, ladder(), 0, 20); // z → active again
        assert!(is_open(&zm, z) && !is_candidate(&zm, z), "re-entry clears candidacy");
    }

    #[test]
    fn soft_release_when_retention_pays_a_refetch() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        // Open active at t=0 and stream a 1000-byte baseline inside the load window.
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        zm.note_update(z, 1000, 0);
        // Pan so z is only warm → soft-held.
        zm.set_anchor("a", 44, 8, ladder(), 0, 10);
        assert!(is_candidate(&zm, z));

        // Past the load window, deltas now charge retention. Under the 1000-byte
        // re-fetch estimate it stays held...
        zm.note_update(z, 400, LOAD_SETTLE_MS + 1);
        zm.note_update(z, 400, LOAD_SETTLE_MS + 2);
        assert!(is_open(&zm, z), "800 < 1000 load — still worth holding");
        // ...the delta that pushes retention to a whole re-fetch releases it.
        zm.note_update(z, 400, LOAD_SETTLE_MS + 3);
        assert!(!is_open(&zm, z), "1200 >= 1000 — retention paid a re-fetch, released");
    }

    #[test]
    fn quiet_soft_held_sub_is_never_released() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        zm.note_update(z, 1000, 0); // baseline load
        zm.set_anchor("a", 44, 8, ladder(), 0, 10); // soft-held, then silent
        // No further rows: a quiet zone (static tiles/things) is held indefinitely.
        assert!(is_open(&zm, z), "a quiet soft-held sub is never released by cost");
    }

    #[test]
    fn fast_pan_by_does_not_instant_drop() {
        // A zone opened then panned past *before* its baseline arrives must not drop
        // the instant the baseline lands: bytes inside the load window are re-transmit
        // cost, not retention, even while soft-held.
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, ladder(), 0, 0); // open active
        zm.set_anchor("a", 44, 8, ladder(), 0, 5); // soft-held at t=5, still no data
        assert!(is_candidate(&zm, z));
        zm.note_update(z, 1000, 50); // baseline lands at t=50 (< LOAD_SETTLE_MS)
        assert!(is_open(&zm, z), "baseline is load, not retention — stays held");
    }

    #[test]
    fn capacity_evicts_candidate_not_hard_held() {
        let mut zm = ZoneManager::new(1);
        let za = zone(0, 0);
        let zb = zone(0, 5);
        // Two distinct active zones via two anchors — both hard-held, cap exceeded
        // but neither evictable.
        zm.set_anchor("a", 8, 8, AnchorRadii { active: 4, ..Default::default() }, 0, 0);
        zm.set_anchor("b", 8, 88, AnchorRadii { active: 4, ..Default::default() }, 0, 0);
        assert!(is_open(&zm, za) && is_open(&zm, zb), "hard-held subs survive over cap");

        // Demote a → candidate; now the cap reclaims it, keeping the hard-held b.
        zm.set_anchor(
            "a",
            44,
            8,
            AnchorRadii { active: 4, warm: 40, ..Default::default() },
            0,
            10,
        );
        assert!(!is_open(&zm, za), "candidate evicted under cap");
        assert!(is_open(&zm, zb), "hard-held kept");
    }

    #[test]
    fn set_anchor_is_idempotent() {
        let mut zm = ZoneManager::default();
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        let _ = zm.take_intents();
        zm.set_anchor("a", 8, 8, ladder(), 0, 1); // identical
        assert!(zm.take_intents().is_empty(), "unchanged anchor emits nothing");
    }

    #[test]
    fn remove_anchor_soft_holds_its_zones() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, ladder(), 0, 0);
        assert!(is_open(&zm, z) && !is_candidate(&zm, z));
        // Removing the anchor uncovers z — but, like panning away, that soft-holds it
        // rather than dropping it (cost / capacity still reclaim it later).
        zm.remove_anchor("a", 10);
        assert!(is_open(&zm, z) && is_candidate(&zm, z), "uncovered on removal → soft-held");
    }

    #[test]
    fn intents_describe_open_then_cost_close() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        // Opening the active zone emits an open intent.
        zm.set_anchor("a", 8, 8, AnchorRadii { active: 4, ..Default::default() }, 0, 0);
        assert_eq!(zm.take_intents(), vec![ZoneIntent { zone_id: z, on: true }]);

        // Stream a baseline, pan away so z soft-holds (the move opens a fresh active
        // zone at the new spot — drain that), then let retention reach the re-fetch
        // estimate: that, and only that, emits z's close intent.
        zm.note_update(z, 500, 0);
        zm.set_anchor("a", 44, 8, AnchorRadii { active: 4, warm: 40, ..Default::default() }, 0, 10);
        let panned = zm.take_intents();
        assert!(!panned.contains(&ZoneIntent { zone_id: z, on: false }), "pan-away does not close z");
        zm.note_update(z, 500, LOAD_SETTLE_MS + 1);
        assert_eq!(zm.take_intents(), vec![ZoneIntent { zone_id: z, on: false }]);
    }
}
