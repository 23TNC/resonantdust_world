//! The composed **world view** — what is at each cell, for every host (shared-simulation P2b).
//!
//! `client/core`'s intent doc specifies it as *"the game client as a headless Rust library — all
//! logic, no rendering"*, with the display *"a dumb display layered over it"* and **"one contract
//! for every host"**. The composed view is the clearest place that was not true: core emitted
//! `ColdTiles`/`ColdThings` and retained nothing, so `client/npc` built one composed view in Rust
//! and `client/webgl` built a second in TypeScript. Two maps of the same world, in two languages,
//! free to disagree — and they did.
//!
//! This is npc's implementation pulled up, unchanged in behaviour, so there is one.
//!
//! **The split it deliberately preserves.** This is the composed MODEL — what kind is at a cell,
//! and whether a pawn may stand there. It is *not* the prim cache: what a viewer draws there,
//! at what zoom, in which atlas page, stays in webgl as a view derived from this. The line is the
//! stream's F2 — the server must agree about the model; only the viewer cares about the drawing.

use std::collections::HashMap;

/// **THE unstreamed-cell law** (pathfinding: pathability is derived, unknown is OPEN).
///
/// A client only knows the zones it has subscribed. A cell outside them is not "a wall" — it is
/// unseen, and treating unseen as blocked would make every pawn refuse to path toward anything
/// beyond its own streamed window, which is worse and wronger than occasionally routing into
/// something the server then clamps.
///
/// Pinned here as one named constant because it was previously answered independently by each
/// host, which is how two clients come to disagree about the same tile.
pub const UNKNOWN_CELL_IS_PATHABLE: bool = true;

/// The composed view: baseline tiles ⊕ cold overlays, plus the composed thing layer.
#[derive(Debug, Default)]
pub struct WorldView {
    /// Zone → 256 `kind_reference` slots (index = `pack_tile_reference(x, y)`), the layer-0
    /// `ColdTiles` baseline.
    tiles: HashMap<u16, Vec<u16>>,
    /// Cold OVERLAY overrides (built walls and the like): `(zone, cell) → kind_reference`.
    tile_overlays: HashMap<(u16, u8), u16>,
    /// The composed THING layer: `(zone, cell) → kind_reference`. A stored **0 means
    /// SUPPRESSED** — a felled tree or eaten meat vanishes from the scans — which is why this
    /// cannot be a plain "absent = nothing" map.
    things: HashMap<(u16, u8), u16>,
}

impl WorldView {
    pub fn new() -> Self {
        Self::default()
    }

    /// A `ColdTiles` baseline row. Only layer 0 is the ground the walk cares about.
    ///
    /// Rows **merge cell-wise, nonzero winning** — a zone streams ONE ROW PER BIOME (subtype),
    /// each carrying only its own cells (seen live: zone 99 arrives as 3 rows). Replacing instead
    /// of merging silently keeps whichever biome streamed last and blanks the rest.
    pub fn observe_cold_tiles(&mut self, zone: u16, layer_id: u8, tiles: &[u16]) {
        if layer_id != 0 {
            return;
        }
        let slot = self.tiles.entry(zone).or_insert_with(|| vec![0; 256]);
        for (i, &kr) in tiles.iter().enumerate().take(slot.len()) {
            if kr != 0 {
                slot[i] = kr;
            }
        }
    }

    /// One cold OVERLAY cell — a built wall, a change to the baseline.
    pub fn observe_tile_overlay(&mut self, zone: u16, cell: u8, kind_reference: u16) {
        self.tile_overlays.insert((zone, cell), kind_reference);
    }

    /// That overlay was removed — the baseline shows through again.
    pub fn remove_tile_overlay(&mut self, zone: u16, cell: u8) {
        self.tile_overlays.remove(&(zone, cell));
    }

    /// A THING **baseline** cell (`ColdThings`). First write wins: a baseline arriving after an
    /// override must not resurrect a felled tree.
    pub fn observe_thing_baseline(&mut self, zone: u16, cell: u8, kind_reference: u16) {
        if kind_reference != 0 {
            self.things.entry((zone, cell)).or_insert(kind_reference);
        }
    }

    /// A THING **override** — it WINS the cell. `0` SUPPRESSES: a felled tree or eaten meat
    /// vanishes from the scans, which is why absent and zero cannot mean the same thing.
    pub fn observe_thing(&mut self, zone: u16, cell: u8, kind_reference: u16) {
        self.things.insert((zone, cell), kind_reference);
    }

    /// A zone left the subscription — drop everything keyed to it, or the view slowly becomes a
    /// memory of places the client can no longer see change.
    pub fn forget_zone(&mut self, zone: u16) {
        self.tiles.remove(&zone);
        self.tile_overlays.retain(|&(z, _), _| z != zone);
        self.things.retain(|&(z, _), _| z != zone);
    }

    pub fn known_zones(&self) -> usize {
        self.tiles.len()
    }

    /// The zone + cell a world tile falls in, if its zone has streamed.
    fn locate(&self, at: (i32, i32)) -> Option<(u16, u8, &Vec<u16>)> {
        use resonantdust_codec::object as obj;
        for (&zone, slots) in &self.tiles {
            let (ox, oy) = obj::macro_world_origin(zone);
            let (dx, dy) = (at.0 - ox, at.1 - oy);
            if (0..16).contains(&dx) && (0..16).contains(&dy) {
                return Some((zone, obj::pack_tile_reference(dx as u8, dy as u8), slots));
            }
        }
        None
    }

    /// The composed `kind_id` (baseline ⊕ overlay) at a world tile, or `None` if its zone has not
    /// streamed. `None` is "unseen", never "empty" — see [`UNKNOWN_CELL_IS_PATHABLE`].
    pub fn tile_kind_at(&self, at: (i32, i32)) -> Option<u16> {
        let (zone, cell, slots) = self.locate(at)?;
        let kr = self
            .tile_overlays
            .get(&(zone, cell))
            .copied()
            .unwrap_or_else(|| slots.get(cell as usize).copied().unwrap_or(0));
        (kr != 0).then_some(kr >> 4)
    }

    /// The composed THING `kind_id` at a world tile — `None` for nothing there or unseen.
    pub fn thing_kind_at(&self, at: (i32, i32)) -> Option<u16> {
        let (zone, cell, _) = self.locate(at)?;
        let kr = self.things.get(&(zone, cell)).copied()?;
        (kr != 0).then_some(kr >> 4)
    }

    /// **Can a pawn stand here.** Derived, never stored: the corpus's own flags applied to the
    /// composed view — the same two `Bundle` predicates the worker's mirror uses, so the answer
    /// cannot drift by being computed differently.
    ///
    /// A thing that blocks beats an open floor; an unseen cell follows
    /// [`UNKNOWN_CELL_IS_PATHABLE`].
    pub fn pathable(&self, bundle: &resonantdust_content::loader::Bundle, x: i32, y: i32) -> bool {
        let Some(kind) = self.tile_kind_at((x, y)) else { return UNKNOWN_CELL_IS_PATHABLE };
        if !bundle.tile_pathable(kind) {
            return false;
        }
        match self.thing_kind_at((x, y)) {
            Some(thing) => bundle.thing_pathable(thing),
            None => true,
        }
    }

    /// The nearest known tile (Chebyshev) whose composed kind satisfies `pred` — a brain's
    /// "where is the nearest water".
    ///
    /// The predicate is handed `&self`. That is not convenience: a caller holding this view
    /// behind a lock would otherwise reach back through its own handle to ask a follow-up
    /// question and **deadlock on a non-reentrant mutex** — which is exactly what happened when
    /// npc first read core's model (I9). Passing the view in makes re-entry fail to compile.
    pub fn nearest_tile(
        &self,
        from: (i32, i32),
        pred: impl Fn(&Self, u16) -> bool,
    ) -> Option<(i32, i32)> {
        self.nearest(from, |v, at| v.tile_kind_at(at).is_some_and(|k| pred(v, k)))
    }

    /// The nearest known THING cell whose composed kind AND world cell satisfy `pred`.
    ///
    /// The CELL is in the predicate on purpose: a brain must be able to refuse UNREACHABLE food
    /// (a drowned pawn's meat in the lake). The worker refuses impathable destinations, and a
    /// brain that keeps picking one oscillates forever between the refusal and its wander.
    /// The predicate is handed `&self` for the reason [`WorldView::nearest_tile`] gives.
    pub fn nearest_thing(
        &self,
        from: (i32, i32),
        pred: impl Fn(&Self, (i32, i32), u16) -> bool,
    ) -> Option<(i32, i32)> {
        self.nearest(from, |v, at| v.thing_kind_at(at).is_some_and(|k| pred(v, at, k)))
    }

    /// Shared scan over every streamed cell, Chebyshev-nearest wins. Ties break on the lowest
    /// `(x, y)` so two hosts scanning the same view pick the SAME cell — an arbitrary-but-stable
    /// tie is the difference between a reproducible brain and a coin flip.
    fn nearest(
        &self,
        from: (i32, i32),
        hit: impl Fn(&Self, (i32, i32)) -> bool,
    ) -> Option<(i32, i32)> {
        use resonantdust_codec::object as obj;
        let mut best: Option<((i32, i32), i32)> = None;
        for &zone in self.tiles.keys() {
            let (ox, oy) = obj::macro_world_origin(zone);
            for dx in 0..16 {
                for dy in 0..16 {
                    let at = (ox + dx, oy + dy);
                    if !hit(self, at) {
                        continue;
                    }
                    let d = (at.0 - from.0).abs().max((at.1 - from.1).abs());
                    let better = match best {
                        None => true,
                        Some((b, bd)) => d < bd || (d == bd && at < b),
                    };
                    if better {
                        best = Some((at, d));
                    }
                }
            }
        }
        best.map(|(at, _)| at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resonantdust_codec::object as obj;

    /// Zone 0's origin is (0,0); fill it with `kind_reference` `kr` everywhere.
    fn zone0(kr: u16) -> WorldView {
        let mut v = WorldView::new();
        v.observe_cold_tiles(0, 0, &vec![kr; 256]);
        v
    }

    /// A zone streams one row per BIOME, each carrying only its own cells. Rows must merge —
    /// replacing keeps whichever biome arrived last and blanks the others.
    #[test]
    fn cold_tile_rows_merge_cell_wise() {
        let mut v = WorldView::new();
        let mut a = vec![0u16; 256];
        a[obj::pack_tile_reference(1, 1) as usize] = 0x10;
        let mut b = vec![0u16; 256];
        b[obj::pack_tile_reference(2, 2) as usize] = 0x20;
        v.observe_cold_tiles(0, 0, &a);
        v.observe_cold_tiles(0, 0, &b);
        assert_eq!(v.tile_kind_at((1, 1)), Some(1), "the first biome row survived the second");
        assert_eq!(v.tile_kind_at((2, 2)), Some(2));
    }

    /// A baseline arriving after an override must not resurrect a felled tree.
    #[test]
    fn a_thing_baseline_never_clobbers_an_override() {
        let mut v = zone0(0x10);
        v.observe_thing(0, 5, 0); // felled
        v.observe_thing_baseline(0, 5, 0x50); // the baseline streams in late
        assert_eq!(v.thing_kind_at((5, 0)), None, "still felled");
    }

    #[test]
    fn an_overlay_beats_the_baseline() {
        let mut v = zone0(0x10); // kind 1
        assert_eq!(v.tile_kind_at((3, 4)), Some(1));
        v.observe_tile_overlay(0, obj::pack_tile_reference(3, 4), 0x70); // kind 7
        assert_eq!(v.tile_kind_at((3, 4)), Some(7));
        assert_eq!(v.tile_kind_at((3, 5)), Some(1), "only the overlaid cell changes");
    }

    /// A stored 0 SUPPRESSES — the felled tree. This is why absent and zero cannot mean the
    /// same thing in the thing layer.
    #[test]
    fn a_zero_thing_suppresses_rather_than_meaning_absent() {
        let mut v = zone0(0x10);
        v.observe_thing(0, obj::pack_tile_reference(2, 2), 0x50);
        assert_eq!(v.thing_kind_at((2, 2)), Some(5));
        v.observe_thing(0, obj::pack_tile_reference(2, 2), 0);
        assert_eq!(v.thing_kind_at((2, 2)), None);
    }

    /// An unseen cell is unseen, not a wall — the law this module pins.
    #[test]
    fn an_unstreamed_cell_is_unknown_not_empty() {
        let v = zone0(0x10);
        assert_eq!(v.tile_kind_at((999, 999)), None);
        assert!(UNKNOWN_CELL_IS_PATHABLE, "the law both hosts must now share");
    }

    /// A zone leaving the subscription takes its overlays and things with it, or the view
    /// remembers a world it can no longer see change.
    #[test]
    fn forgetting_a_zone_drops_everything_keyed_to_it() {
        let mut v = zone0(0x10);
        v.observe_tile_overlay(0, 5, 0x70);
        v.observe_thing(0, 5, 0x50);
        v.forget_zone(0);
        assert_eq!(v.known_zones(), 0);
        assert_eq!(v.tile_kind_at((0, 0)), None);
        assert_eq!(v.thing_kind_at((0, 0)), None);
    }

    /// Nearest is Chebyshev with a stable tie — two hosts scanning one view must agree on WHICH
    /// cell, not merely on the distance.
    #[test]
    fn nearest_is_chebyshev_with_a_stable_tie() {
        let mut v = zone0(0x10);
        for (x, y) in [(5u8, 5u8), (5, 7)] {
            v.observe_thing(0, obj::pack_tile_reference(x, y), 0x50);
        }
        assert_eq!(v.nearest_thing((5, 6), |_, _, k| k == 5), Some((5, 5)), "equidistant → lowest (x,y)");
        assert_eq!(v.nearest_thing((5, 9), |_, _, k| k == 5), Some((5, 7)));
        assert_eq!(v.nearest_thing((5, 6), |_, _, k| k == 99), None);
    }
}
