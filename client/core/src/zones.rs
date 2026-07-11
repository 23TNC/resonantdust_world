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
//!   - **warm** — *sticky*. Still open, but now a close-candidate: eligible for
//!     warmth eviction (drop on update-noise) and the capacity-LRU.
//!   - **cold** (or uncovered) — *drop*. The sub closes.
//!
//! Moving outward a sub walks active → hot → warm → cold = wanted → wanted →
//! candidate → closed; it only ever *opens* by crossing into active. Two
//! thresholds (active to enter, cold to leave) with a soft eviction band between.
//!
//! ## Sticky eviction
//!
//! A candidate (warm) sub does not close the instant it stops being hard-held.
//! Network re-transmit is the binding cost, so we hold generously and drop only
//! the noisy or over-budget:
//!   - **warmth** ([`should_close`], run on each inbound update — a silent
//!     candidate is never evicted by warmth), and
//!   - **capacity-LRU** ([`ZoneManager::enforce_capacity`]) — a hard ceiling
//!     ([`DEFAULT_MAX_OPEN_SUBS`]); the least-recently-demoted candidate goes
//!     first, and a hard-held (active/hot) sub is never evicted.
//!
//! Sans-IO: decisions become [`ZoneIntent`]s the engine drains and maps to
//! `sub_zone` / `unsub` frames. `now` (ms) is threaded in for warmth recency, so
//! the manager stays pure and unit-testable.

use std::collections::{HashMap, HashSet};

use resonantdust_codec::packed::{pack_zone_id, REGION_DIM, ZONE_DIM};

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
    surface: u8,
    radii: AnchorRadii,
    /// The soul (pawn card id) this anchor represents, or `0` for a non-soul
    /// anchor such as a viewport. Reserved for the future per-soul memory path;
    /// unused by subscription logic today.
    soul: u32,
}

/// Live state for one open zone subscription. A `Sub` exists in the map iff we
/// believe a `SubZone` is open for it; closing removes the entry.
#[derive(Debug, Clone, Copy)]
struct Sub {
    /// The tier last assigned while open. `Active`/`Hot` are hard-held;
    /// `Warm`/`Cold` mean the sub is a candidate (see `candidate_since_ms`).
    tier: ZoneTier,
    /// `Some(ms)` once the sub became a close-candidate (entered warm), reset to
    /// `None` whenever it re-hardens (back to active/hot). Drives warmth aging
    /// and capacity-LRU ordering.
    candidate_since_ms: Option<u64>,
    /// Inbound updates seen since this sub became a candidate; compared against
    /// the warmth tolerance. Reset on each (re-)entry to the candidate state.
    updates_since_candidate: u32,
}

/// Warmth eviction policy: should a candidate close after `updates` inbound row
/// changes, having been a candidate for `age_ms`? Recently-demoted subs tolerate
/// more churn; old ones close on any update. A silent candidate (0 updates) is
/// never closed here — only capacity-LRU reclaims it.
fn should_close(updates: u32, age_ms: u64) -> bool {
    let tolerance = if age_ms < 5 * 60_000 {
        5
    } else if age_ms < 15 * 60_000 {
        3
    } else {
        0
    };
    updates > tolerance
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
        surface: u8,
        radii: AnchorRadii,
        soul: u32,
        now: u64,
    ) {
        let anchor = Anchor {
            tile_x,
            tile_y,
            surface,
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

    /// Record an inbound row change for `zone_id`. Ages the zone's candidacy (if
    /// it is a candidate) and may evict it via warmth. Hard-held subs ignore this.
    pub fn note_update(&mut self, zone_id: u32, now: u64) {
        let close = match self.subs.get_mut(&zone_id) {
            Some(s) => match s.candidate_since_ms {
                Some(since) => {
                    s.updates_since_candidate += 1;
                    should_close(s.updates_since_candidate, now.saturating_sub(since))
                }
                None => false,
            },
            None => false,
        };
        if close {
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
                        self.open_sub(zone);
                    }
                    self.harden(zone, ZoneTier::Active);
                }
                // Hysteresis hold: sustain an open sub; never open a fresh one.
                Some(ZoneTier::Hot) => {
                    if open {
                        self.harden(zone, ZoneTier::Hot);
                    }
                }
                // Sticky: sustain as an evictable candidate; never open fresh.
                Some(ZoneTier::Warm) => {
                    if open {
                        self.make_candidate(zone, now);
                    }
                }
                // Out of range: drop.
                Some(ZoneTier::Cold) | None => {
                    if open {
                        self.close_sub(zone);
                    }
                }
            }
        }

        self.enforce_capacity();
    }

    /// Open a sub for `zone` (inserted hard-held at active; the caller assigns the
    /// final tier) and emit the intent.
    fn open_sub(&mut self, zone: u32) {
        self.subs.insert(
            zone,
            Sub {
                tier: ZoneTier::Active,
                candidate_since_ms: None,
                updates_since_candidate: 0,
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

    /// Mark an open sub a warm close-candidate. Stamps the demotion time (and
    /// resets the update counter) only on the transition into candidacy, so a sub
    /// that stays warm keeps aging from when it first demoted.
    fn make_candidate(&mut self, zone: u32, now: u64) {
        if let Some(s) = self.subs.get_mut(&zone) {
            s.tier = ZoneTier::Warm;
            if s.candidate_since_ms.is_none() {
                s.candidate_since_ms = Some(now);
                s.updates_since_candidate = 0;
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
                if let Some(zone) = zone_at(zgx, zgy, anchor.surface) {
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
/// Region cells per world axis (`region_x`/`region_y` are full bytes: 0..256).
const REGION_AXIS: i32 = 256;
/// Zones per world axis — the valid range for a global zone coordinate.
const ZONES_PER_AXIS: i32 = REGION_AXIS * REGION_EDGE;

/// The global zone coordinate containing global tile coordinate `tile` (floor
/// division, so it is correct for off-origin tiles).
fn zone_axis(tile: i32) -> i32 {
    tile.div_euclid(ZONE_EDGE)
}

/// Pack a global zone coordinate `(zgx, zgy)` on `surface` into a `zone_id`, or
/// `None` if it falls outside the world.
fn zone_at(zgx: i32, zgy: i32, surface: u8) -> Option<u32> {
    if !(0..ZONES_PER_AXIS).contains(&zgx) || !(0..ZONES_PER_AXIS).contains(&zgy) {
        return None;
    }
    let region_x = (zgx / REGION_EDGE) as u8;
    let zone_x = (zgx % REGION_EDGE) as u8;
    let region_y = (zgy / REGION_EDGE) as u8;
    let zone_y = (zgy % REGION_EDGE) as u8;
    Some(pack_zone_id(region_x, region_y, surface, zone_x, zone_y))
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
        zone_at(zgx, zgy, 0).unwrap()
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
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        let desired = zm.desired_tiers();
        assert_eq!(desired.get(&zone(0, 0)), Some(&ZoneTier::Active));
        assert_eq!(desired.get(&zone(1, 0)), Some(&ZoneTier::Hot));
        assert_eq!(desired.get(&zone(2, 0)), Some(&ZoneTier::Warm));
        assert_eq!(desired.get(&zone(4, 0)), Some(&ZoneTier::Cold));
    }

    #[test]
    fn only_active_opens_a_sub() {
        let mut zm = ZoneManager::default();
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        // The active zone opens; the hot/warm/cold ring does not (hot+ only hold).
        assert!(is_open(&zm, zone(0, 0)));
        assert!(!is_open(&zm, zone(1, 0)));
        assert!(!is_open(&zm, zone(2, 0)));
        assert_eq!(zm.open_sub_count(), 1);
    }

    #[test]
    fn hysteresis_holds_then_drops() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);

        // Open at active.
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        assert!(is_open(&zm, z) && !is_candidate(&zm, z));

        // Move so z is now only Hot: still open, still hard-held.
        zm.set_anchor("a", 28, 8, 0, ladder(), 0, 10);
        assert!(is_open(&zm, z) && !is_candidate(&zm, z), "hot sustains hard");

        // Move so z is now Warm: still open, now a candidate.
        zm.set_anchor("a", 44, 8, 0, ladder(), 0, 20);
        assert!(is_open(&zm, z) && is_candidate(&zm, z), "warm = sticky candidate");

        // Move so z is out of every band: closed.
        zm.set_anchor("a", 88, 8, 0, ladder(), 0, 30);
        assert!(!is_open(&zm, z), "cold/uncovered drops");
    }

    #[test]
    fn reentering_active_rehardens() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        zm.set_anchor("a", 44, 8, 0, ladder(), 0, 10); // z → warm candidate
        assert!(is_candidate(&zm, z));
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 20); // z → active again
        assert!(is_open(&zm, z) && !is_candidate(&zm, z), "re-entry clears candidacy");
    }

    #[test]
    fn warmth_evicts_a_noisy_candidate() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 1000); // open active
        zm.set_anchor("a", 44, 8, 0, ladder(), 0, 1000); // demote to warm candidate
        assert!(is_candidate(&zm, z));

        // Fresh candidate tolerates 5 updates.
        for _ in 0..5 {
            zm.note_update(z, 1000);
        }
        assert!(is_open(&zm, z), "within tolerance, still open");
        // The 6th exceeds it.
        zm.note_update(z, 1000);
        assert!(!is_open(&zm, z), "warmth closed the noisy candidate");
    }

    #[test]
    fn silent_candidate_survives_warmth() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        zm.set_anchor("a", 44, 8, 0, ladder(), 0, 0); // warm candidate, no updates
        assert!(is_open(&zm, z), "a silent candidate is never closed by warmth");
    }

    #[test]
    fn capacity_evicts_candidate_not_hard_held() {
        let mut zm = ZoneManager::new(1);
        let za = zone(0, 0);
        let zb = zone(0, 5);
        // Two distinct active zones via two anchors — both hard-held, cap exceeded
        // but neither evictable.
        zm.set_anchor("a", 8, 8, 0, AnchorRadii { active: 4, ..Default::default() }, 0, 0);
        zm.set_anchor("b", 8, 88, 0, AnchorRadii { active: 4, ..Default::default() }, 0, 0);
        assert!(is_open(&zm, za) && is_open(&zm, zb), "hard-held subs survive over cap");

        // Demote a → candidate; now the cap reclaims it, keeping the hard-held b.
        zm.set_anchor(
            "a",
            44,
            8,
            0,
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
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        let _ = zm.take_intents();
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 1); // identical
        assert!(zm.take_intents().is_empty(), "unchanged anchor emits nothing");
    }

    #[test]
    fn remove_anchor_closes_its_zones() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, 0, ladder(), 0, 0);
        assert!(is_open(&zm, z));
        zm.remove_anchor("a", 10);
        assert!(!is_open(&zm, z), "removing the last anchor closes its sub");
    }

    #[test]
    fn intents_describe_open_then_close() {
        let mut zm = ZoneManager::default();
        let z = zone(0, 0);
        zm.set_anchor("a", 8, 8, 0, AnchorRadii { active: 4, ..Default::default() }, 0, 0);
        assert_eq!(zm.take_intents(), vec![ZoneIntent { zone_id: z, on: true }]);
        zm.remove_anchor("a", 1);
        assert_eq!(zm.take_intents(), vec![ZoneIntent { zone_id: z, on: false }]);
    }
}
