//! Resonant Dust (world) client-side wasm crate — hello-world scaffold.
//!
//! Compiled to a browser wasm bundle (see the `wasm` service in `compose.yml`)
//! and imported by the webgl client. The server does NOT consume this crate —
//! it links the shared logic crates (`resonantdust-core`, …) directly as rlibs.
//! The bindings live only here, where they're needed.
//!
//! The substance is plain Rust in the member crates — feature-independent, so
//! `cargo check` / `test` exercise it natively without the wasm toolchain. The
//! browser surface is a thin `#[wasm_bindgen]` layer, gated on `js`, that
//! marshals values in/out and delegates to the plain functions.

// Re-export the native API so server-side rlib consumers can depend on
// `resonantdust-shared` and get the underlying crates' surface in one place.
pub use resonantdust_core::greeting;
// The content crate — re-exported so native rlib consumers reach it through this one crate
// too. The browser surface ([`Content`]) wraps it behind the `js` feature.
pub use resonantdust_content as dsl;

#[cfg(feature = "js")]
use wasm_bindgen::prelude::*;

// ---------- Browser surface (js feature) ----------

/// `greeting(name)` → the shared greeting string. The thin JS wrapper: it just
/// delegates to the plain [`greeting`] the server also calls.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = greeting)]
pub fn greeting_js(name: &str) -> String {
    greeting(name)
}

/// The simulation rate (tics/second) — the codec authority (`codec::tic::TIC_HZ`). The client's
/// wall↔tic speculation extrapolates with this.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = ticHz)]
pub fn tic_hz_js() -> u16 {
    resonantdust_codec::tic::TIC_HZ
}

/// The degenerate-fallback hop pace (`codec::speed`). A pawn's real pace is the DERIVED
/// `ground_speed` stat (input-rework F8 — `pawnGroundSpeed`); this covers only the
/// pre-fan window and a 0-derived pawn nothing should be moving anyway.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = defaultTicsPerTile)]
pub fn default_tics_per_tile_js() -> u16 {
    resonantdust_codec::speed::DEFAULT_TICS_PER_TILE
}

/// Pack a global tile into a `position_reference` (`codec::object`) — a console/debug
/// affordance for hand-queueing programs (e.g. minting a posed test pawn with `CREATE`).
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = tileToPosition)]
pub fn tile_to_position_js(tile_x: i32, tile_y: i32) -> u32 {
    resonantdust_codec::object::tile_to_position(tile_x, tile_y)
}

/// Compose an `EXECUTE_INTERACTION` program (interactions F4's layout verbatim:
/// `[op, reference, version, count, inputs…]`) from a resolved gameplay reference + bound
/// input words — the pie menu's send path (`WorldClient.queue` takes the result).
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = composeInteraction)]
pub fn compose_interaction_js(reference: u32, inputs: Vec<u32>) -> Vec<u32> {
    let mut p = vec![
        resonantdust_codec::action::EXECUTE_INTERACTION,
        reference,
        0,
        inputs.len() as u32,
    ];
    p.extend_from_slice(&inputs);
    p
}

/// Compose a `SPAWN_REQUEST` program (spawn-authority F1) — the ONE packer, shared
/// through the codec so the webgl chat and the npc cannot drift. `def`'s variant
/// nibble is ignored; `variants[i]` = part i's u4 (≤4 by the parts law).
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = packSpawnRequest)]
pub fn pack_spawn_request_js(x: u16, y: u16, rotation: u8, def: u32, variants: Vec<u8>) -> Vec<u32> {
    resonantdust_codec::action::pack_spawn_request(x, y, rotation, def, &variants).to_vec()
}

/// A taxonomy flattened for the JS boundary: `[type, subType, kind, variant]`, taking the FIRST
/// entry of each applicability array. Sufficient to key the registry — a def's `kind_id` is shared
/// across every tuple it covers, so any one of them resolves to the same kind.
#[cfg(feature = "js")]
fn taxonomy_row(t: &dsl::loader::Taxonomy) -> Vec<String> {
    vec![
        t.type_name.clone(),
        t.sub_type.first().cloned().unwrap_or_default(),
        t.kind.clone(),
        t.variant.first().cloned().unwrap_or_default(),
    ]
}

// ---------- content runtime (js feature) ----------
//
// The client's view of the content: load the fetched TOML corpus once, then answer
// the per-tile queries the renderer makes. `def_id` is what arrives in a zone's
// packed tile slots (`tileDef` off the codec); `Content` turns it back into the
// `visual.color.bg` the painter fills with. The server loads the SAME corpus from
// disk and derives the SAME ids, so a `def_id` means one tile on both sides.

/// A loaded content corpus — the client's handle to the content bundle. Built from
/// the fetched TOML sources; queried per tile while painting a zone.
#[cfg(feature = "js")]
#[wasm_bindgen]
pub struct Content {
    bundle: dsl::Bundle,
}

#[cfg(feature = "js")]
#[wasm_bindgen]
impl Content {
    /// Load a corpus from parallel `names` / `sources` arrays — one entry per
    /// fetched `*.toml` file (`names` are for error messages only; def ids are
    /// explicit in the TOML, so order carries no meaning). Throws a string of
    /// every parse error if the corpus is bad.
    #[wasm_bindgen(constructor)]
    pub fn new(names: Vec<String>, sources: Vec<String>) -> Result<Content, JsValue> {
        let pairs: Vec<(String, String)> = names.into_iter().zip(sources).collect();
        match dsl::load(&pairs) {
            Ok(bundle) => Ok(Content { bundle }),
            Err(errs) => {
                let msg = errs
                    .iter()
                    .map(|e| format!("{}: {}", e.file, e.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                Err(JsValue::from_str(&msg))
            }
        }
    }

    /// Inject the DEFINITION REGISTRY's `name → kind_id` resolution
    /// (definition-registry P5). Three parallel arrays because that is the cheapest thing to hand
    /// across the wasm boundary: `isTile[i]` / `names[i]` / `kindIds[i]`.
    ///
    /// The registry is the authority for a NAME lookup; a name it does not carry falls back to the
    /// corpus's own id, so a partially seeded registry degrades to today's behaviour rather than to
    /// nothing. Today the two agree by construction (the registry is seeded from those ids) — this
    /// exists so that when the corpus stops carrying numbers, resolution keeps working unchanged.
    #[wasm_bindgen(js_name = withRegistry)]
    pub fn with_registry(&mut self, is_tile: Vec<u8>, names: Vec<String>, defs: Vec<u32>) {
        let mut map = std::collections::HashMap::new();
        for ((t, n), id) in is_tile.into_iter().zip(names).zip(defs) {
            map.insert((t != 0, n), id);
        }
        // `with_registry` consumes; swap through a placeholder-free take/rebuild.
        let bundle = std::mem::take(&mut self.bundle);
        self.bundle = bundle.with_registry(map);
    }

    /// A tile's background colour as `0xRRGGBB`, by packed `def_id` — the
    /// per-cell lookup the painter runs over a zone's tile slots. `undefined`
    /// when the def is unknown or declares no `visual.color.bg`.
    #[wasm_bindgen(js_name = tileColorBg)]
    pub fn tile_color_bg(&self, def_id: u16) -> Option<u32> {
        self.bundle.color_bg_for_def(def_id)
    }

    /// The `def_id` for a tile name (e.g. `"grass"`), or `undefined`.
    #[wasm_bindgen(js_name = tileDefId)]
    pub fn tile_def_id(&self, name: &str) -> Option<u16> {
        self.bundle.tile_def_id(name)
    }

    /// The tile name for a `def_id`, or `undefined` (`0` is the empty sentinel).
    #[wasm_bindgen(js_name = tileName)]
    pub fn tile_name(&self, def_id: u16) -> Option<String> {
        self.bundle.tile_name(def_id).map(str::to_string)
    }

    /// Every tile name, in `def_id` order (index 0 → def_id 1) — for the debug
    /// HUD / a palette legend.
    /// Every THING name, in `object_id` order (index 0 → object_id 1) — the thing-side sibling of
    /// [`Content::tile_names`], needed to bind the definition registry by name.
    #[wasm_bindgen(js_name = thingNames)]
    pub fn thing_names(&self) -> Vec<String> {
        self.bundle.thing_names().to_vec()
    }

    /// A tile's TAXONOMY as `[type, subType, kind, variant]`, or `undefined` if it authors none.
    /// The first `subType`/`variant` of the applicability arrays — enough to key the registry,
    /// since a def's `kind_id` is the same across every tuple it covers.
    #[wasm_bindgen(js_name = tileTaxonomy)]
    pub fn tile_taxonomy(&self, def_id: u16) -> Option<Vec<String>> {
        self.bundle.tile_taxonomy(def_id).map(taxonomy_row)
    }

    /// A thing's TAXONOMY as `[type, subType, kind, variant]`, or `undefined`.
    #[wasm_bindgen(js_name = thingTaxonomy)]
    pub fn thing_taxonomy(&self, object_id: u16) -> Option<Vec<String>> {
        self.bundle.thing_taxonomy(object_id).map(taxonomy_row)
    }

    #[wasm_bindgen(js_name = tileNames)]
    pub fn tile_names(&self) -> Vec<String> {
        self.bundle.tile_names().to_vec()
    }

    /// Expand a zone's packed tile slots into renderable prims — the painter's
    /// per-zone call. Runs the codec (cell → coords, slot → `def_id`) and the content
    /// (`def_id` → its `:visual @on_create` [`VisualParts`]) for every non-empty
    /// cell, returning a flat **stride-5** array `[tileX, tileY, tint, geoColor,
    /// defId, …]` in *global tile coordinates*. `tint` multiplies a loaded sprite;
    /// `geoColor` is the flat silhouette shown until it loads; `defId` indexes
    /// [`tileTextureStems`] for the sprite stem (empty = flat tint, no sprite).
    /// Empty cells (`def_id == 0`) are skipped. One wasm-boundary crossing per
    /// zone, not 256.
    #[wasm_bindgen(js_name = zoneTilePrims)]
    pub fn zone_tile_prims(&self, macro_position: u16, tiles: Vec<u16>) -> Vec<f64> {
        use resonantdust_codec::object;
        let (origin_x, origin_y) = macro_origin(macro_position);
        let mut out = Vec::new();
        for (i, &kind) in tiles.iter().enumerate() {
            if kind == 0 {
                continue; // empty cell — no floor/wall
            }
            let def_id = object::kind_ref_kind_id(kind); // the u16 tile is a kind_reference; def = kind_id
            // The dense index IS a canonical `tile_reference` (`tile_x:4 | tile_y:4`), same as the
            // things layer + worldgen — NOT the legacy `packed::cell` (x in the low nibble), which
            // transposes every zone across its diagonal and shears biomes at the seams.
            let location = i as u8;
            let tile_x = origin_x + object::ref_hi(location) as i64;
            let tile_y = origin_y + object::ref_lo(location) as i64;
            // The corpus resolves the tint + geo colour; unknown defs fall
            // back to a white tint (and geo = tint).
            let visual = self.bundle.visual_for_def(def_id);
            let tint = visual.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF);
            let geo = visual.as_ref().map(|v| v.geo_color).unwrap_or(tint);
            out.push(tile_x as f64);
            out.push(tile_y as f64);
            out.push(tint as f64);
            out.push(geo as f64);
            out.push(def_id as f64);
        }
        out
    }

    /// Every tile's texture stem in `def_id` order (index 0 → def_id 1) — the
    /// stem table the host fetches once and indexes by the `defId` each tile prim
    /// carries. An empty string means "flat tint, no sprite" (the built-in white
    /// fill); a non-empty stem names the sprite the resolver loads.
    #[wasm_bindgen(js_name = tileTextureStems)]
    pub fn tile_texture_stems(&self) -> Vec<String> {
        self.bundle.tile_texture_stems()
    }

    /// Every tile's BUILD CATEGORY in `def_id` order (empty = not buildable) — the
    /// table the build menu scans (build-walls D3: content alone lights the icons).
    #[wasm_bindgen(js_name = tileBuilds)]
    pub fn tile_builds(&self) -> Vec<String> {
        self.bundle.tile_builds()
    }

    /// Every tile's HEIGHT in `def_id` order (0 = flat) — the cold-lighting participation
    /// gate (tile-lighting F2: > 0 ⇒ receiver + caster records).
    #[wasm_bindgen(js_name = tileHeights)]
    pub fn tile_heights(&self) -> Vec<f64> {
        self.bundle.tile_heights()
    }

    /// Every tile's lighting/linked lanes, flat stride-6 per def_id:
    /// `[linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows]`
    /// (texture-generalization P0 — content is the authority, never the async manifest).
    #[wasm_bindgen(js_name = tileLightingLanes)]
    pub fn tile_lighting_lanes(&self) -> Vec<f64> {
        self.bundle.tile_lighting_lanes()
    }

    /// Every thing's texture stem in `object_id` order — the thing-layer sibling
    /// of [`tileTextureStems`], indexed by the `defId` a thing / free-thing prim
    /// carries.
    #[wasm_bindgen(js_name = thingTextureStems)]
    pub fn thing_texture_stems(&self) -> Vec<String> {
        self.bundle.thing_texture_stems()
    }

    /// Every thing's spatial LAYOUT in `object_id` order, indexed by the `defId` a
    /// thing / free-thing prim carries. Flat **stride-7** per def: `[footprint.w,
    /// footprint.h, anchor.x, anchor.y, size, sprite_anchor.x, sprite_anchor.y]`
    /// (footprint + size in tiles, anchors in `0..1`). A def with no prim yields the
    /// default row `[1, 1, 0.5, 0.5, 1, 0.5, 0.5]`. The host (`WorldBridge`/`MoverLayer`)
    /// resolves it into a world-px box (via `SQUARE`) + a z-row.
    #[wasm_bindgen(js_name = thingLayout)]
    pub fn thing_layout(&self) -> Vec<f64> {
        self.bundle.thing_layout()
    }

    /// Every thing's SUBFRAMES in `object_id` order — flat **stride-1536** per def: 16 VARIANTS ×
    /// 16 ROTATIONS × `[sub.x, sub.y, sub.w, sub.h, anchor.x, anchor.y]`, fractions in `0..1`
    /// (subframe-ingest I10).
    ///
    /// The rect the atlas CROPS to: ingest copies exactly this out of each of the stem's four maps,
    /// so albedo/normal/surface/layers are registered with each other by construction. A def that
    /// authors nothing yields the whole frame `(0,0,1,1)` three times, so adopting the subframe
    /// cannot move art that has not opted in.
    ///
    /// **Two axes, because every variation has rotations.** The ROTATION is `0..15` — a facing
    /// (`0 = s, 1 = e, 2 = n, 3 = w`) or a linked autotile cell (`y·4 + x`) — and it lands in the
    /// STEM. The VARIANT is the other `0..15` and lands in the CELL. Index 3 (west) is the east
    /// master mirrored, derived host-side from index 1, never authored.
    #[wasm_bindgen(js_name = thingSubframe)]
    pub fn thing_subframe(&self) -> Vec<f64> {
        self.bundle.thing_subframe()
    }

    // ── needs & conditions (needs-moodlets P3/F3) — the ONE eval, reached through wasm ──

    /// A pawn's ACTIVE conditions at `now_tic`, evaluated from its raw payload sidecar
    /// (`NEED` + `CONDITION` entries — `TABLES.md § payload`). Flat **stride-4** per active
    /// condition: `[condition_id, magnitude_sum, remaining_tics, priority]` (`remaining 0` =
    /// DERIVED, alive while its band holds). The SAME `needs_eval` the npc Brain imports
    /// natively — the panel and the Brain cannot disagree on a crossing by construction.
    ///
    /// **Already sorted** (conditions F3, emotions F4) — `priority` desc, Σ emotion magnitude
    /// desc, `condition_id` asc. The caller renders in the order given; `priority` rides along
    /// so it can be shown, not so it can be re-sorted. A TS sort here would be the second
    /// implementation of a corpus rule.
    /// `needs` is the pawn's `needs` SUB-TABLE rows, flattened stride-2:
    /// `[packed_row, set_tic, …]` (stat-model F2); traits + stored conditions decode from
    /// `payload`. The eval is the SAME `needs_eval` the npc and worker import natively.
    #[wasm_bindgen(js_name = pawnConditions)]
    pub fn pawn_conditions(
        &self,
        kind: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
    ) -> Vec<f64> {
        let (traits, conditions) = decode_payload(&self.bundle, kind, &payload);
        let need_rows = decode_need_rows(&needs);
        let active = resonantdust_content::needs_eval::active_conditions(
            &self.bundle, &traits, &need_rows, &conditions, now_tic,
        );
        let mut out = Vec::with_capacity(active.len() * 4);
        for c in &active {
            out.extend_from_slice(&[
                f64::from(c.condition_id),
                f64::from(c.magnitude_sum),
                f64::from(c.remaining),
                f64::from(c.priority),
            ]);
        }
        out
    }

    /// The pawn's ACTIVE EMOTION at `now_tic` (emotions F3): flat 17 f64s —
    /// `[active_index, sum0..sum15]` (index = the u4 declaration order; empty → 0 = fine).
    /// The ONE `emotion_eval` argmax every consumer shares.
    #[wasm_bindgen(js_name = pawnEmotion)]
    pub fn pawn_emotion(
        &self,
        kind: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
    ) -> Vec<f64> {
        let (traits, conditions) = decode_payload(&self.bundle, kind, &payload);
        let need_rows = decode_need_rows(&needs);
        let active = resonantdust_content::needs_eval::active_conditions(
            &self.bundle, &traits, &need_rows, &conditions, now_tic,
        );
        let (e, sums) =
            resonantdust_content::emotion_eval::active_emotion(&self.bundle, &traits, &active);
        let mut out = Vec::with_capacity(17);
        out.push(f64::from(e));
        out.extend(sums.iter().map(|&s| f64::from(s)));
        out
    }

    /// One condition's emotion modifiers (emotions F2), for the card's pie slices and the
    /// tooltip: flat stride-2 `[emotion_index, magnitude, …]` in authored order. Empty for
    /// an unknown ref or a modifier-free condition (the `fine +0` gray card).
    #[wasm_bindgen(js_name = conditionEmotions)]
    pub fn condition_emotions(&self, condition_id: u32) -> Vec<f64> {
        let Some(cp) = self.bundle.condition_params_by_ref(condition_id) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(cp.emotions.len() * 2);
        for m in &cp.emotions {
            out.extend_from_slice(&[f64::from(m.emotion), f64::from(m.magnitude)]);
        }
        out
    }

    /// The emotion table in declaration (u4) order (emotions F1): flat stride-2
    /// `[index, color_0xRRGGBB, …]`. Labels come via [`Self::emotion_label`].
    #[wasm_bindgen(js_name = emotionTable)]
    pub fn emotion_table(&self) -> Vec<f64> {
        let all = self.bundle.emotion_params_all();
        let mut out = Vec::with_capacity(all.len() * 2);
        for (i, (_, p)) in all.iter().enumerate() {
            out.extend_from_slice(&[i as f64, f64::from(p.color)]);
        }
        out
    }

    /// An emotion's display label by u4 index ("" = unknown).
    #[wasm_bindgen(js_name = emotionLabel)]
    pub fn emotion_label(&self, index: u8) -> String {
        self.bundle
            .emotion_params_all()
            .get(index as usize)
            .map(|(_, p)| p.label.clone())
            .unwrap_or_default()
    }

    /// An emotion's `0xRRGGBB` color by u4 index (−1 = unknown).
    #[wasm_bindgen(js_name = emotionColor)]
    pub fn emotion_color(&self, index: u8) -> f64 {
        self.bundle
            .emotion_params_all()
            .get(index as usize)
            .map(|(_, p)| f64::from(p.color))
            .unwrap_or(-1.0)
    }

    // ── pathability + the shared pathfinder (pathfinding F1/F2) ──

    /// May a pawn ENTER this tile kind? Unknown/retired ids read OPEN (the Bundle's guard).
    #[wasm_bindgen(js_name = tilePathable)]
    pub fn tile_pathable(&self, def_id: u16) -> bool {
        self.bundle.tile_pathable(def_id)
    }

    /// May a pawn ENTER a cell this thing kind occupies? Unknown ids read OPEN.
    #[wasm_bindgen(js_name = thingPathable)]
    pub fn thing_pathable(&self, object_id: u16) -> bool {
        self.bundle.thing_pathable(object_id)
    }

    /// The SHARED path (pathfinding F2/I1) over a WINDOWED pathability grid: `cells` is
    /// row-major `w × h` (1 = enterable) anchored at world tile `(ox, oy)`; outside the
    /// window reads CLOSED (the window must carry a margin — the caller's tradeoff for a
    /// bounded copy; the worker paths the unbounded mirror, so a route hugging the window
    /// edge can diverge until the next authoritative correction). Returns flat
    /// `[x0, y0, x1, y1, …]` EXCLUSIVE of the start, empty when no path (or already there).
    #[wasm_bindgen(js_name = findPath)]
    pub fn find_path(
        &self,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        ox: i32,
        oy: i32,
        w: u32,
        h: u32,
        cells: Vec<u8>,
    ) -> Vec<i32> {
        let probe = |x: i32, y: i32| -> bool {
            let (dx, dy) = (x - ox, y - oy);
            if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                return false;
            }
            cells
                .get((dy as usize) * (w as usize) + dx as usize)
                .is_some_and(|&b| b != 0)
        };
        match resonantdust_content::path_eval::find_path((from_x, from_y), (to_x, to_y), &probe)
        {
            Some(p) => p.into_iter().flat_map(|(x, y)| [x, y]).collect(),
            None => Vec::new(),
        }
    }

    /// The SHARED chord polyline (chord-movement F5) over the same windowed grid as
    /// [`Self::find_path`] — flat `[x0, y0, x1, y1, …]` waypoints exclusive of the start,
    /// empty when no route (or already there). The speculation glides THESE.
    #[wasm_bindgen(js_name = findChords)]
    #[allow(clippy::too_many_arguments)]
    pub fn find_chords(
        &self,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        ox: i32,
        oy: i32,
        w: u32,
        h: u32,
        cells: Vec<u8>,
    ) -> Vec<i32> {
        let probe = |x: i32, y: i32| -> bool {
            let (dx, dy) = (x - ox, y - oy);
            if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                return false;
            }
            cells
                .get((dy as usize) * (w as usize) + dx as usize)
                .is_some_and(|&b| b != 0)
        };
        match resonantdust_content::path_eval::find_chords(
            (from_x, from_y),
            (to_x, to_y),
            0,
            &probe,
        ) {
            Some(p) => p.into_iter().flat_map(|(x, y)| [x, y]).collect(),
            None => Vec::new(),
        }
    }

    /// The next FUTURE tic the pawn's active-condition set can change WITHOUT a new write
    /// (band crossing under the PIECEWISE rate, or a stored row's expiry), or `-1` when
    /// nothing ahead changes — what lets the panel re-evaluate on a schedule (F4).
    #[wasm_bindgen(js_name = pawnNextCrossing)]
    pub fn pawn_next_crossing(
        &self,
        kind: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
    ) -> f64 {
        let (traits, conditions) = decode_payload(&self.bundle, kind, &payload);
        let need_rows = decode_need_rows(&needs);
        resonantdust_content::needs_eval::next_crossing_tic(
            &self.bundle, &traits, &need_rows, &conditions, now_tic,
        )
        .map_or(-1.0, f64::from)
    }

    /// A condition's display LABEL by its u32 gameplay `definition_reference` — the panel's
    /// name lookup (interactions F1: the eval returns REFS, so a positional label array
    /// cannot index them). `undefined` for an unknown ref. Labels are presentation.
    #[wasm_bindgen(js_name = conditionLabelOf)]
    pub fn condition_label_of(&self, reference: u32) -> Option<String> {
        self.bundle.condition_params_by_ref(reference).map(|m| m.label)
    }

    /// A condition's need-modifier lines for the everything-tooltip (emotions F6), one
    /// pre-formatted string per modifier — `"thirst rate ×0.5"` (+ ` min N` / ` max N`
    /// when authored). Empty for an unknown ref or a modifier-free condition.
    #[wasm_bindgen(js_name = conditionNeedLines)]
    pub fn condition_need_lines(&self, reference: u32) -> Vec<String> {
        let Some(cp) = self.bundle.condition_params_by_ref(reference) else { return Vec::new() };
        cp.needs
            .iter()
            .map(|n| {
                let mut line = format!("{} rate \u{00d7}{}", n.need, n.rate);
                if let Some(m) = n.min {
                    line.push_str(&format!(" min {m}"));
                }
                if let Some(m) = n.max {
                    line.push_str(&format!(" max {m}"));
                }
                line
            })
            .collect()
    }

    /// Bind the GAMEPLAY registry (interactions F1): three parallel arrays over the
    /// `/definitions` rows whose `type` is `"gameplay"` — `categories[i]` = the row's
    /// `sub_type`, `names[i]` = its `kind`, `defs[i]` = the u32 id. Without this the bundle
    /// resolves through the corpus-position SEED, which a fresh registry reproduces exactly.
    #[wasm_bindgen(js_name = withGameplayRegistry)]
    pub fn with_gameplay_registry(&mut self, categories: Vec<String>, names: Vec<String>, defs: Vec<u32>) {
        let mut map = std::collections::HashMap::new();
        for ((c, n), id) in categories.into_iter().zip(names).zip(defs) {
            map.insert((c, n), id);
        }
        let bundle = std::mem::take(&mut self.bundle);
        self.bundle = bundle.with_gameplay_registry(map);
    }

    /// Per-kind emitted light, stride-8 (`[r, g, b, intensity, reach, radius, height, flags]`;
    /// flags bit 0 = cast_shadows, bit 1 = hot). `reach == 0` ⇒ the kind emits no light.
    #[wasm_bindgen(js_name = thingLight)]
    pub fn thing_light(&self) -> Vec<f64> {
        self.bundle.thing_light()
    }

    /// A pawn's DERIVED `ground_speed` in **tics per tile** (input-rework F8): the ONE
    /// value speculation must share with the worker's continuation spacing, computed from
    /// the pawn's fanned rows (`payload` + stride-2 `needs`) through the shared
    /// `stat_eval`. `0` = no walks / rows not fanned yet — the host falls back to
    /// `defaultTicsPerTile`.
    #[wasm_bindgen(js_name = pawnGroundSpeed)]
    pub fn pawn_ground_speed(
        &self,
        kind: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
    ) -> f64 {
        let (traits, conditions) = decode_payload(&self.bundle, kind, &payload);
        let need_rows = decode_need_rows(&needs);
        let active = resonantdust_content::needs_eval::active_conditions(
            &self.bundle, &traits, &need_rows, &conditions, now_tic,
        );
        resonantdust_content::stat_eval::stat_value(&self.bundle, "ground_speed", &traits, &active)
    }

    /// The pie-menu options for a clicked TILE (input-rework F1/F4/F5): the tile def's
    /// carried interactions, filtered by the acting pawn's affordance PREDICATES (its
    /// fanned rows in — the SAME `interaction_available` the worker enforces) and the
    /// LOCATION rule via the shared `location_in_range` (`cheb_distance` = pawn↔clicked
    /// cell in tiles — lumberjack F2). Returns an array of
    /// `{ name, menuText, reference, magnitude, inputs }` objects; empty = no menu.
    #[wasm_bindgen(js_name = tileMenuOptions)]
    pub fn tile_menu_options(
        &self,
        tile_def_id: u16,
        pawn_kind: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
        cheb_distance: u32,
    ) -> js_sys::Array {
        menu_options(&self.bundle, self.bundle.tile_interactions(tile_def_id), pawn_kind, payload, needs, now_tic, cheb_distance)
    }

    /// The same for a clicked THING's kind (`object_id`) — trees offer `cut_down` here
    /// (lumberjack F6); a waterskin stays a TOML edit.
    #[wasm_bindgen(js_name = thingMenuOptions)]
    pub fn thing_menu_options(
        &self,
        object_id: u16,
        pawn_kind: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
        cheb_distance: u32,
    ) -> js_sys::Array {
        menu_options(&self.bundle, self.bundle.thing_interactions(object_id), pawn_kind, payload, needs, now_tic, cheb_distance)
    }

    /// The object's emitted LIGHTS (trait-lights F4/F8) through THE merged accessor —
    /// flat **stride 9** per light: `[r, g, b, intensity, reach, fall_off, elevation,
    /// radius, flags]` (`flags` bit 0 = cast, bit 1 = hot, bit 2 = flicker). Empty =
    /// nothing on this object emits. Payload optional (cold things pass none).
    #[wasm_bindgen(js_name = objectLights)]
    pub fn object_lights_js(&self, kind: u16, payload: Vec<u32>) -> Vec<f64> {
        let rows = resonantdust_codec::payload::payload_traits(&payload);
        let lights = self.bundle.object_lights(kind, &rows);
        let mut out = Vec::with_capacity(lights.len() * 9);
        for l in lights {
            out.extend_from_slice(&[
                l.color.0, l.color.1, l.color.2, l.intensity, l.reach, l.fall_off,
                l.elevation, l.radius,
                f64::from(u8::from(l.cast) | (u8::from(l.hot) << 1) | (u8::from(l.flicker) << 2)),
            ]);
        }
        out
    }

    /// A gameplay def's u32 `definition_reference` (registry-first, seed fallback) — the
    /// menu's event composer resolves interaction refs through this.
    #[wasm_bindgen(js_name = gameplayReference)]
    pub fn gameplay_reference_js(&self, category: String, name: String) -> Option<u32> {
        self.bundle.gameplay_reference(&category, &name)
    }

    /// Does `object_id`'s thing def carry the named NEED (inventory F7)? The [Inventory]
    /// button and panel gate on `kindHasNeed(kind, "inventory")` — resolved by NAME so
    /// registry/seed drift cannot bite.
    #[wasm_bindgen(js_name = kindHasNeed)]
    pub fn kind_has_need(&self, object_id: u16, need: String) -> bool {
        self.bundle.thing_needs(object_id).iter().any(|r| {
            self.bundle.gameplay_lookup(*r).is_some_and(|(c, n)| c == "need" && n == need)
        })
    }

    /// The EFFECTIVE max of a need for a pawn's rows (`need_bounds` hi — the leveled-trait
    /// cap): the inventory panel's GRID SIZE (inventory F7; a level-2 trait widens it).
    #[wasm_bindgen(js_name = needMax)]
    pub fn need_max(
        &self,
        kind: u16,
        payload: Vec<u32>,
        need: String,
        now_tic: u16,
    ) -> Option<f64> {
        let (traits, conditions) = decode_payload(&self.bundle, kind, &payload);
        let np = self.bundle.need_params(&need)?;
        let (_, hi) = resonantdust_content::needs_eval::need_bounds(
            &self.bundle, &need, &np, &traits, &conditions, now_tic,
        );
        Some(hi)
    }

    /// The SLOT pie-menu options (inventory F5): the acting pawn's OWN kind's
    /// slot-located interactions (drop today), availability-filtered by the SAME rules
    /// as every other menu (`location_in_range("slot", ·)` is definitionally in range).
    #[wasm_bindgen(js_name = slotMenuOptions)]
    pub fn slot_menu_options(
        &self,
        pawn_object_id: u16,
        payload: Vec<u32>,
        needs: Vec<u32>,
        now_tic: u16,
    ) -> js_sys::Array {
        let binds: Vec<_> = self
            .bundle
            .thing_interactions(pawn_object_id)
            .into_iter()
            .filter(|b| {
                self.bundle.interaction_params(&b.name).is_some_and(|ip| ip.location == "slot")
            })
            .collect();
        menu_options(&self.bundle, binds, pawn_object_id, payload, needs, now_tic, 0)
    }

    /// A thing's placeholder/background colour by `object_id` (`0xRRGGBB`), or `None` —
    /// the inventory panel's slot fill (inventory F7).
    #[wasm_bindgen(js_name = thingColor)]
    pub fn thing_color(&self, object_id: u16) -> Option<u32> {
        self.bundle.thing_color_for_object(object_id)
    }

    /// An interaction's queue-strip visuals by its `definition_reference`
    /// (intent-queue-ui F2): `{ hover, size, background, progress, progressColor,
    /// progressFill, cancelable }`. `hover` falls back to the LABEL; colors are
    /// `0xRRGGBB` numbers or null (the panel's neutral). Unknown ref = null.
    #[wasm_bindgen(js_name = queueVisual)]
    pub fn queue_visual(&self, reference: u32) -> JsValue {
        let Some((cat, name)) = self.bundle.gameplay_lookup(reference) else {
            return JsValue::NULL;
        };
        if cat != "interaction" {
            return JsValue::NULL;
        }
        let Some(ip) = self.bundle.interaction_params(&name) else { return JsValue::NULL };
        let obj = js_sys::Object::new();
        let set = |key: &str, value: &JsValue| {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(key), value);
        };
        let q = &ip.queue;
        set("hover", &JsValue::from_str(q.hover.as_deref().unwrap_or(&ip.label)));
        set("size", &JsValue::from_f64(q.size));
        match q.background {
            Some(c) => set("background", &JsValue::from_f64(f64::from(c))),
            None => set("background", &JsValue::NULL),
        }
        set("progress", &JsValue::from_str(&q.progress));
        match q.progress_color {
            Some(c) => set("progressColor", &JsValue::from_f64(f64::from(c))),
            None => set("progressColor", &JsValue::NULL),
        }
        set("progressFill", &JsValue::from_bool(q.progress_fill));
        set("cancelable", &JsValue::from_bool(q.cancelable));
        obj.into()
    }

    /// Every tile's 4 packed-map channel material bindings, in `def_id` order — a
    /// per-def table the host fetches once and indexes by the `defId` a tile prim
    /// carries (like [`tileTextureStems`]). Flat **stride-8** per def:
    /// `[mat0, tint0, mat1, tint1, mat2, tint2, mat3, tint3]`, where `matN` is a
    /// 1-based [`materialParams`] index (`0` = no material) and `tintN` a `0xRRGGBB`
    /// base colour. All-zero = no material system for that channel (flat albedo).
    #[wasm_bindgen(js_name = tilePackedChannels)]
    pub fn tile_packed_channels(&self) -> Vec<f64> {
        flatten_packed(self.bundle.tile_packed_channels())
    }

    /// Every thing's 4 packed-channel material bindings, in `object_id` order — the
    /// thing-layer sibling of [`tilePackedChannels`].
    #[wasm_bindgen(js_name = thingPackedChannels)]
    pub fn thing_packed_channels(&self) -> Vec<f64> {
        flatten_packed(self.bundle.thing_packed_channels())
    }

    /// The material registry's noise-field NAMES, in `material_id` order (index 0 →
    /// id 1). The host resolves each to a noise-atlas index. Empty = no field (flat).
    #[wasm_bindgen(js_name = materialNoiseFields)]
    pub fn material_noise_fields(&self) -> Vec<String> {
        self.bundle.material_params_all().into_iter().map(|m| m.noise_field).collect()
    }

    /// The material registry's sample spaces (`"uv"` | `"world"`), in `material_id`
    /// order — the parallel-array sibling of [`materialNoiseFields`].
    #[wasm_bindgen(js_name = materialSampleSpaces)]
    pub fn material_sample_spaces(&self) -> Vec<String> {
        self.bundle.material_params_all().into_iter().map(|m| m.sample_space).collect()
    }

    /// The material registry's numeric swings, flat **stride-3** in `material_id`
    /// order: `[hueSwing (deg), chromaSwing, warmCoolBias, …]`. The host binds these
    /// as the bake shader's per-material jitter uniforms.
    #[wasm_bindgen(js_name = materialSwings)]
    pub fn material_swings(&self) -> Vec<f64> {
        let mut out = Vec::new();
        for m in self.bundle.material_params_all() {
            out.push(m.hue_swing);
            out.push(m.chroma_swing);
            out.push(m.warm_cool_bias);
        }
        out
    }

    /// NORMAL-DETAIL field names in `material_id` order (material-system P1) — the
    /// parallel-array sibling of [`materialNoiseFields`]; `""` = no detail.
    #[wasm_bindgen(js_name = materialDetailFields)]
    pub fn material_detail_fields(&self) -> Vec<String> {
        self.bundle.material_params_all().into_iter().map(|m| m.detail_field).collect()
    }

    /// NORMAL-DETAIL numerics, flat **stride-2** in `material_id` order:
    /// `[detailAmp, detailScale, …]`. Amp 0 = identity (no detail).
    #[wasm_bindgen(js_name = materialDetail)]
    pub fn material_detail(&self) -> Vec<f64> {
        let mut out = Vec::new();
        for m in self.bundle.material_params_all() {
            out.push(m.detail_amp);
            out.push(m.detail_scale);
        }
        out
    }

    /// Expand a zone's cold-object row (a module's `cold` table — the OBJECT MODEL) into
    /// renderable prims. `type_reference` is the row's shared `object_type_reference`
    /// (type / subtype = biome / layer); each `kinds` entry is a `u32`
    /// `object_kind_reference` (`kind:10 | subkind:4 | variant:4 | x:4 | y:4 | data:6`).
    /// Dispatches the visual by the row's `type_id` — `biome-tile` → the tile namespace,
    /// `biome-thing` → the thing namespace — and returns a flat **stride-7** array
    /// `[tileX, tileY, tint, geoColor, kindId, data, variant]` in global tile coordinates
    /// (the same shape as [`zoneThingPrims`]; a `biome-tile` prim's first 5 fields match
    /// [`zoneTilePrims`]). The host picks the stem table + painter by [`objectTypeId`];
    /// `kindId` indexes that namespace's stems, `variant` the sprite. One boundary
    /// crossing per cold row. Unifies the legacy [`zoneTilePrims`]/[`zoneThingPrims`].
    #[wasm_bindgen(js_name = zoneColdPrims)]
    pub fn zone_cold_prims(&self, macro_position: u16, type_id: u8, kinds: Vec<u32>) -> Vec<f64> {
        use resonantdust_codec::object;
        let (origin_x, origin_y) = macro_origin(macro_position);
        // `type_id` is the cold shard (the frame: `biome-tile` vs `biome-thing`) — not read off a
        // row field anymore. It only selects the sprite namespace here.
        let is_tile = type_id == object::TYPE_BIOME_TILE;
        let mut out = Vec::new();
        for &k in &kinds {
            let kind_id = object::kind_pos_ref_kind_id(k);
            let tile_x = origin_x + object::kind_pos_ref_x(k) as i64;
            let tile_y = origin_y + object::kind_pos_ref_y(k) as i64;
            // biome-tile kinds live in the TILE namespace (def_id), biome-thing kinds in
            // the THING namespace (object_id).
            let visual =
                if is_tile { self.bundle.visual_for_def(kind_id) } else { self.bundle.visual_for_object(kind_id) };
            let tint = visual.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF);
            let geo = visual.as_ref().map(|v| v.geo_color).unwrap_or(tint);
            out.push(tile_x as f64);
            out.push(tile_y as f64);
            out.push(tint as f64);
            out.push(geo as f64);
            out.push(kind_id as f64);
            out.push(object::kind_pos_ref_data(k) as f64);
            out.push(object::kind_pos_ref_variant_id(k) as f64);
        }
        out
    }

    /// The `type_id` of a cold row's `type_reference` — the host reads it to route a
    /// `coldObjects` row to its ground painter ([`Content::type_biome_tile`]) or thing
    /// painter ([`Content::type_biome_thing`]) and pick the matching stem table.
    #[wasm_bindgen(js_name = objectTypeId)]
    pub fn object_type_id(&self, type_reference: u16) -> u8 {
        resonantdust_codec::object::type_ref_type_id(type_reference)
    }

    /// The `type_id` for biome-classified ground tiles (`biome-tile`).
    #[wasm_bindgen(js_name = typeBiomeTile)]
    pub fn type_biome_tile(&self) -> u8 {
        resonantdust_codec::object::TYPE_BIOME_TILE
    }

    /// The `type_id` for biome-scattered things (`biome-thing`).
    #[wasm_bindgen(js_name = typeBiomeThing)]
    pub fn type_biome_thing(&self) -> u8 {
        resonantdust_codec::object::TYPE_BIOME_THING
    }

    /// A pawn KIND's part SLOTS (human-pawns P2) — the authored skeleton MoverLayer renders,
    /// one JS object per `^prim call` in the kind's visual: `{stem, part, scale, offsetX,
    /// offsetY, offsetZ, depth, size, span, anchorX, anchorY, spriteAnchorX, spriteAnchorY,
    /// tint, geoColor}`.
    /// The wolf yields 1 slot, a human 2 (body + head). An unknown kind yields a single
    /// default slot (white fill), so the caller never branches on emptiness. Replaces the
    /// old `moverPrim` (position now comes from the state row alone).
    #[wasm_bindgen(js_name = moverParts)]
    pub fn mover_parts(&self, kind: u16) -> JsValue {
        let arr = js_sys::Array::new();
        let push = |arr: &js_sys::Array, p: &resonantdust_content::loader::VisualPart| {
            let o = js_sys::Object::new();
            let set = |k: &str, v: &JsValue| {
                let _ = js_sys::Reflect::set(&o, &JsValue::from_str(k), v);
            };
            match &p.texture {
                Some(s) => set("stem", &JsValue::from_str(s)),
                None => set("stem", &JsValue::NULL),
            }
            set("part", &JsValue::from_f64(p.part as f64));
            set("scale", &JsValue::from_f64(p.scale));
            set("offsetX", &JsValue::from_f64(p.offset.0));
            set("offsetY", &JsValue::from_f64(p.offset.1));
            // z-positioning P3: HEIGHT off the ground, in tiles. The renderer shifts the part
            // north 1:1 so it draws where an equal `offsetY` would, but the shadow system keeps
            // the parent's ground footprint -- which is what makes body and head cast ONE
            // aligned shadow instead of two.
            set("offsetZ", &JsValue::from_f64(p.elevation));
            set("depth", &JsValue::from_f64(p.depth));
            set("size", &JsValue::from_f64(p.size));
            set("span", &JsValue::from_f64(p.span));
            set("anchorX", &JsValue::from_f64(p.anchor.0));
            set("anchorY", &JsValue::from_f64(p.anchor.1));
            set("spriteAnchorX", &JsValue::from_f64(p.sprite_anchor.0));
            set("spriteAnchorY", &JsValue::from_f64(p.sprite_anchor.1));
            // subframe-ingest I10: the slot's OWN crop rects, flat **stride-1536** — 16 VARIANTS ×
            // 16 ROTATIONS × `[x, y, w, h, anchorX, anchorY]`. Two axes, not one: a facing lands in
            // the STEM and a variant in the CELL, so collapsing them put the wolf's south rect on
            // its east art. Same shape `thingSubframe` uses, so the host has one decoder for both.
            let mut sf = Vec::with_capacity(16 * 16 * 6);
            for by_rot in p.dir_frames.iter() {
                for f in by_rot.iter() {
                    sf.extend_from_slice(&[f.sub.0, f.sub.1, f.sub.2, f.sub.3, f.anchor.0, f.anchor.1]);
                }
            }
            set("subframes", &js_sys::Float64Array::from(&sf[..]).into());
            set("tint", &JsValue::from_f64(p.tint as f64));
            set("geoColor", &JsValue::from_f64(p.geo_color as f64));
            arr.push(&o);
        };
        match self.bundle.visual_for_object(kind) {
            Some(v) if !v.parts.is_empty() => {
                for p in &v.parts {
                    push(&arr, p);
                }
            }
            v => {
                // No visual (or a pre-parts one): a single default slot from the flat fields.
                let d = resonantdust_content::loader::VisualPart {
                    tint: v.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF),
                    geo_color: v.as_ref().map(|v| v.geo_color).unwrap_or(0x00FF_FFFF),
                    texture: v.as_ref().and_then(|v| v.texture.clone()),
                    part: 0,
                    scale: 1.0,
                    offset: (0.0, 0.0),
                    elevation: 0.0,   // a default slot stands on the ground
                    depth: 0.0,
                    size: v.as_ref().map(|v| v.size).unwrap_or(1.0),
                    span: v.as_ref().map(|v| v.span).unwrap_or(1.0),
                    sprite_scale: v.as_ref().map(|v| v.sprite_scale).unwrap_or((1.0, 1.0)),
                    sprite_anchor: v.as_ref().map(|v| v.sprite_anchor).unwrap_or((0.5, 0.5)),
                    anchor: v.as_ref().map(|v| v.anchor).unwrap_or((0.5, 0.5)),
                    // A pre-parts kind carries its subframe on the flat prim-0 fields; anything
                    // with no visual at all gets the whole frame, which crops nothing.
                    dir_frames: v.as_ref().map(|v| v.dir_frames).unwrap_or_default(),
                };
                push(&arr, &d);
            }
        }
        arr.into()
    }

    /// Render data for one **cold overlay** cell (a cold `state` row the host composites over the
    /// baseline). Decodes the `position_reference` → the cell, and `definition_reference` → the sprite
    /// (`type_id` picks tile vs thing namespace, `kind_id`/`variant` the sprite). Returns
    /// `[tileX, tileY, tint, geoColor, kindId, typeId, data, variant]` — stride 8. An empty vec (`kind
    /// == 0`) means "nothing here" (a removal that leaves the cell bare).
    #[wasm_bindgen(js_name = coldStatePrim)]
    pub fn cold_state_prim(
        &self,
        macro_position: u16,
        position_reference: u32,
        definition_reference: u32,
        data: u8,
    ) -> Vec<f64> {
        use resonantdust_codec::object;
        let kind_id = object::def_kind_id(definition_reference);
        if kind_id == 0 {
            return Vec::new(); // empty override — the cell renders bare (baseline suppressed)
        }
        let (origin_x, origin_y) = macro_origin(macro_position);
        let tile = object::micro_position_tile(object::position_micro(position_reference));
        let tile_x = origin_x + object::ref_hi(tile) as i64;
        let tile_y = origin_y + object::ref_lo(tile) as i64;
        let type_id = object::def_type_id(definition_reference);
        let variant = object::def_variant_id(definition_reference);
        let is_tile = type_id == object::TYPE_BIOME_TILE;
        let visual =
            if is_tile { self.bundle.visual_for_def(kind_id) } else { self.bundle.visual_for_object(kind_id) };
        let tint = visual.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF);
        let geo = visual.as_ref().map(|v| v.geo_color).unwrap_or(tint);
        vec![
            tile_x as f64,
            tile_y as f64,
            tint as f64,
            geo as f64,
            kind_id as f64,
            type_id as f64,
            data as f64,
            variant as f64,
        ]
    }

    /// The **cell + type** of a cold overlay, independent of its `kind` — so a *removal* override
    /// (`kind == 0`, for which [`Content::cold_state_prim`] returns empty) still names the baseline
    /// cell it suppresses. Returns `[tileX, tileY, typeId]`. `type_id` (tile vs thing) comes from
    /// `definition_reference`; the cell from `position_reference`'s `tile_reference`.
    #[wasm_bindgen(js_name = coldCell)]
    pub fn cold_cell(&self, macro_position: u16, position_reference: u32, definition_reference: u32) -> Vec<f64> {
        use resonantdust_codec::object;
        let (origin_x, origin_y) = macro_origin(macro_position);
        let tile = object::micro_position_tile(object::position_micro(position_reference));
        let tile_x = origin_x + object::ref_hi(tile) as i64;
        let tile_y = origin_y + object::ref_lo(tile) as i64;
        let type_id = object::def_type_id(definition_reference);
        vec![tile_x as f64, tile_y as f64, type_id as f64]
    }
}

/// A macro position's origin in **global tile coordinates**: its region + zone
/// nibbles, each scaled by the region/zone edge in tiles. The shared prefix of
/// every prim-expansion call ([`Content::zone_tile_prims`], `zone_cold_prims`,
/// `mover_prim`). Straight from `object::macro_world_origin` — the same helper
/// worldgen samples, so generation and render place a cell identically.
#[cfg(feature = "js")]
/// The pie-menu filter (input-rework F1/F4/F5), ONE implementation for tile and thing
/// carriers: bindings → location rule → the shared predicate gate → option objects.
#[cfg(feature = "js")]
fn menu_options(
    bundle: &dsl::loader::Bundle,
    binds: Vec<dsl::loader::InteractionBind>,
    pawn_kind: u16,
    payload: Vec<u32>,
    needs: Vec<u32>,
    now_tic: u16,
    cheb_distance: u32,
) -> js_sys::Array {
    let (traits, conditions) = decode_payload(bundle, pawn_kind, &payload);
    let need_rows = decode_need_rows(&needs);
    let active = resonantdust_content::needs_eval::active_conditions(
        bundle, &traits, &need_rows, &conditions, now_tic,
    );
    let out = js_sys::Array::new();
    for dsl::loader::InteractionBind { name, magnitude, .. } in binds {
        let Some(ip) = bundle.interaction_params(&name) else { continue };
        // The location rule (F4 / lumberjack F2): the SHARED range check — the menu must
        // never offer what the worker would refuse. RELAXED for signatures that bind a
        // `destination` (lumberjack P3) or a `target` PAWN (attack F2 — the worker walks
        // the actor to the victim's live position): the worker composes [walk, act]
        // through the intent queue for those, so distance no longer refuses; a signature
        // with neither has nowhere to walk and stays strictly ranged.
        let can_compose_walk = ip.inputs.iter().any(|n| n == "destination" || n == "target");
        if !can_compose_walk && !dsl::loader::location_in_range(&ip.location, cheb_distance) {
            continue;
        }
        if !resonantdust_content::stat_eval::interaction_available(
            bundle, &name, &traits, &need_rows, &conditions, &active, now_tic,
        ) {
            continue;
        }
        let Some(reference) = bundle.gameplay_reference("interaction", &name) else { continue };
        let obj = js_sys::Object::new();
        let set = |key: &str, value: &JsValue| {
            let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(key), value);
        };
        set("name", &JsValue::from_str(&name));
        set("menuText", &JsValue::from_str(&ip.menu_text));
        set("reference", &JsValue::from_f64(f64::from(reference)));
        set("magnitude", &JsValue::from_f64(magnitude));
        let inputs = js_sys::Array::new();
        for i in &ip.inputs {
            inputs.push(&JsValue::from_str(i));
        }
        set("inputs", &inputs.into());
        out.push(&obj.into());
    }
    out
}

/// Decode a pawn payload into its (trait rows, condition rows) — the stat-model shapes.
/// Trait rows go through THE merged accessor (trait-lights F5): the kind's CONSTANT
/// binds derive here beside the payload's runtime rows, so every eval on this side of
/// the wasm boundary sees the same traits the worker and npc do.
fn decode_payload(
    bundle: &dsl::loader::Bundle,
    kind: u16,
    payload: &[u32],
) -> (Vec<u32>, Vec<(u32, u16)>) {
    (
        bundle.object_trait_rows(kind, &resonantdust_codec::payload::payload_traits(payload)),
        resonantdust_codec::payload::payload_conditions(payload),
    )
}

/// Un-flatten the stride-2 `[packed_row, set_tic, …]` needs rows the host passes.
fn decode_need_rows(needs: &[u32]) -> Vec<(u32, u16)> {
    needs.chunks_exact(2).map(|c| (c[0], c[1] as u16)).collect()
}

fn macro_origin(macro_position: u16) -> (i64, i64) {
    let (ox, oy) = resonantdust_codec::object::macro_world_origin(macro_position);
    (ox as i64, oy as i64)
}

/// Flatten a per-def table of 4 packed channels into the **stride-8** array the
/// host indexes by `def_id`: `[mat0, tint0, …, mat3, tint3]` per def (see
/// [`Content::tile_packed_channels`]).
#[cfg(feature = "js")]
fn flatten_packed(table: Vec<[dsl::loader::PackedChannel; 4]>) -> Vec<f64> {
    let mut out = Vec::with_capacity(table.len() * 8);
    for channels in table {
        for ch in channels {
            out.push(ch.material_id as f64);
            out.push(ch.tint as f64);
        }
    }
    out
}

// ---------- world client (js feature) ----------
//
// The browser's handle to the world: login + the anchor-driven zone
// subscription engine, wrapping the `client` crate's `web` transport. webgl
// constructs one with the gateway base + an event callback, then drives it with
// `login` / `setAnchor` — the same `Command` verbs the native headless driver
// sends. Every `Event` the engine emits is marshaled to a small tagged JS object
// and handed to the callback.

/// The webgl-facing world client. Owns the session; events arrive on the
/// callback passed to [`new`](WorldClient::new).
#[cfg(feature = "js")]
#[wasm_bindgen]
pub struct WorldClient {
    inner: client::Client,
}

#[cfg(feature = "js")]
#[wasm_bindgen]
impl WorldClient {
    /// Construct a client pointed at `gateway_url` (the gateway HTTP base, no
    /// trailing `/server`). `on_event` is called with one tagged object per
    /// [`Event`](client::Event) — `{ kind, … }` (see [`event_to_js`]).
    #[wasm_bindgen(constructor)]
    pub fn new(gateway_url: String, on_event: js_sys::Function) -> WorldClient {
        let config = client::ClientConfig::for_gateway(gateway_url);
        let inner = client::Client::spawn(config, move |event: client::Event| {
            let payload = event_to_js(&event);
            // The host callback shouldn't throw; ignore a JS exception if it does.
            let _ = on_event.call1(&JsValue::NULL, &payload);
        });
        WorldClient { inner }
    }

    /// Log in as `name` (trust-on-first-use). Drives the login event sequence.
    pub fn login(&self, name: String) {
        let _ = self.inner.login(name);
    }

    /// Add or move the viewport anchor `name` to global tile `(tile_x, tile_y)`, with the four
    /// hysteresis-tier reaches in tiles. `soul` is `0` for a viewport. Idempotent — cheap to call
    /// every pan. (The world is a single 2D tile plane per the geographic model; the old
    /// `surface`/z-axis is retired.)
    #[wasm_bindgen(js_name = setAnchor)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_anchor(
        &self,
        name: String,
        tile_x: i32,
        tile_y: i32,
        active: i32,
        hot: i32,
        warm: i32,
        cold: i32,
        soul: u32,
    ) {
        let radii = client::AnchorRadii {
            active,
            hot,
            warm,
            cold,
        };
        let _ = self.inner.set_anchor(name, tile_x, tile_y, radii, soul);
    }

    /// Remove the anchor `name`, closing any subscriptions only it held.
    #[wasm_bindgen(js_name = removeAnchor)]
    pub fn remove_anchor(&self, name: String) {
        let _ = self.inner.remove_anchor(name);
    }

    /// Queue a raw action program (`docs/ACTIONS.md`) into the simulation — the general world door.
    /// No-op before login / if disconnected.
    #[wasm_bindgen(js_name = queue)]
    pub fn queue(&self, actions: Vec<u32>) {
        let _ = self.inner.queue(actions);
    }

    /// Move `entity` (an `entity_reference`) toward global tile `(tile_x, tile_y)` — compiles to a
    /// `MOVE_TO` program. No-op before login / if disconnected.
    // `moveEntity` DIED with the raw MOVE_TO door (input-rework F3): the host composes
    // `composeInteraction(move_to_ref, [pawn, destination])` and sends it through `queue`.

    /// Order walls on the `(start..end)` tile rect's PERIMETER (build-walls D5) — compiles to a
    /// `BUILD_WALL` program; the worker expands + queues the per-tile SETs. No-op offline.
    #[wasm_bindgen(js_name = buildWall)]
    pub fn build_wall(&self, start_x: i32, start_y: i32, end_x: i32, end_y: i32, object: u32) {
        let _ = self.inner.build_wall(start_x, start_y, end_x, end_y, object);
    }

    /// Seed the tic estimator's rate from a persisted hint (movement-hardening F5 — kills the
    /// cold-page warmup). Clamped; ignored once the stream has anchored.
    #[wasm_bindgen(js_name = seedTicRate)]
    pub fn seed_tic_rate(&self, tics_per_sec: f64) {
        let _ = self.inner.seed_tic_rate(tics_per_sec);
    }

    /// Place + promote `entity` at global tile `(tile_x, tile_y)` — compiles to a `PROMOTE_STATE` +
    /// `PLACE` program (the spawn path until `CREATE`'s minted-id claim lands). No-op before login.
    #[wasm_bindgen(js_name = place)]
    pub fn place(&self, entity: u32, tile_x: i32, tile_y: i32) {
        let _ = self.inner.place(entity, tile_x, tile_y);
    }

    /// Drop the world-server connection, keeping the client alive for reconnect.
    pub fn logout(&self) {
        let _ = self.inner.logout();
    }

    /// Stop the engine entirely.
    pub fn shutdown(&self) {
        let _ = self.inner.shutdown();
    }
}

/// Marshal one [`Event`](client::Event) to a tagged JS object the host switches
/// on by `kind`. `u32` ids go through as `number` (within JS's safe-integer
/// range); a zone's tiles go through as a `Uint8Array`, its things as a `Uint32Array`.
#[cfg(feature = "js")]
fn event_to_js(event: &client::Event) -> JsValue {
    use client::Event;

    let obj = js_sys::Object::new();
    let set = |key: &str, value: &JsValue| {
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(key), value);
    };

    match event {
        Event::LoginStarted { name } => {
            set("kind", &JsValue::from_str("loginStarted"));
            set("name", &JsValue::from_str(name));
        }
        Event::ServerResolved(info) => {
            set("kind", &JsValue::from_str("serverResolved"));
            set("serverId", &JsValue::from_f64(info.server_id as f64));
            set("url", &JsValue::from_str(&info.url));
            set("reused", &JsValue::from_bool(info.reused));
        }
        Event::LoggedIn {
            player_id,
            player_shard_reference,
            server_url,
        } => {
            set("kind", &JsValue::from_str("loggedIn"));
            set("playerId", &JsValue::from_f64(*player_id as f64));
            set(
                "playerShardReference",
                &JsValue::from_f64(*player_shard_reference as f64),
            );
            set("serverUrl", &JsValue::from_str(server_url));
        }
        Event::LoginFailed { reason } => {
            set("kind", &JsValue::from_str("loginFailed"));
            set("reason", &JsValue::from_str(reason));
        }
        Event::Disconnected { reason } => {
            set("kind", &JsValue::from_str("disconnected"));
            match reason {
                Some(r) => set("reason", &JsValue::from_str(r)),
                None => set("reason", &JsValue::NULL),
            }
        }
        Event::Status(message) => {
            // TEMP (logs-drop drill): surface status text race-free (parse errors ride it).
            let arr = js_sys::eval("window.__statusRecv=window.__statusRecv||[];window.__statusRecv")
                .ok()
                .and_then(|v| v.dyn_into::<js_sys::Array>().ok());
            if let Some(arr) = arr {
                arr.push(&JsValue::from_str(message));
            }
            set("kind", &JsValue::from_str("status"));
            set("message", &JsValue::from_str(message));
        }
        Event::StateObject {
            macro_position,
            entity_reference,
            definition_reference,
            tile_x,
            tile_y,
            sub_x,
            sub_y,
            facing,
            tic,
            removed,
        } => {
            set("kind", &JsValue::from_str("stateObject"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            // entity_reference (server_reference:8 | object_reference:24) is a u32 — JS-safe. Its
            // top nibble is the object type; the host keys the mover by the whole reference.
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            // The content kind → sprite (`kind` the JS key is the event discriminator above).
            set("definitionReference", &JsValue::from_f64(*definition_reference as f64));
            set("tileX", &JsValue::from_f64(*tile_x as f64));
            set("tileY", &JsValue::from_f64(*tile_y as f64));
            // Subtile sixteenths (chord-movement F1) — the host composes the FRACTIONAL
            // authoritative point as `tile + sub/16`.
            set("subX", &JsValue::from_f64(*sub_x as f64));
            set("subY", &JsValue::from_f64(*sub_y as f64));
            set("facing", &JsValue::from_f64(*facing as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            set("removed", &JsValue::from_bool(*removed));
        }
        Event::PawnParts { macro_position, entity_reference, tic, parts, payload } => {
            set("kind", &JsValue::from_str("pawnParts"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            // (slot, def) pairs flattened [slot, def, slot, def, …]; defs are u32 — JS-safe.
            let arr = js_sys::Uint32Array::new_with_length((parts.len() * 2) as u32);
            for (i, (slot, def)) in parts.iter().enumerate() {
                arr.set_index((i * 2) as u32, *slot as u32);
                arr.set_index((i * 2 + 1) as u32, *def);
            }
            set("parts", &arr);
            // The RAW opcode stream (needs-moodlets P4) — the panel feeds it straight to the
            // one eval (`Content.pawnConditions`); the host never decodes NEED/CONDITION itself.
            let raw = js_sys::Uint32Array::new_with_length(payload.len() as u32);
            raw.copy_from(payload);
            set("payload", &raw);
        }
        Event::PawnNeed { macro_position, entity_reference, need, set_tic } => {
            set("kind", &JsValue::from_str("pawnNeed"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            // The packed gameplay row (value:16 | kind:12 | variant:4) + the lazy anchor —
            // the host buffers pairs and feeds them to `pawnConditions` untouched.
            set("need", &JsValue::from_f64(*need as f64));
            set("setTic", &JsValue::from_f64(*set_tic as f64));
        }
        Event::PawnInventory { macro_position, entity_reference, slot, item, state } => {
            set("kind", &JsValue::from_str("pawnInventory"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            // One held-item slot (inventory F2): `item` = the thing's definition_reference,
            // 0 = the slot emptied (drop/removal); `state` RESERVED (item-as-entity).
            set("slot", &JsValue::from_f64(*slot as f64));
            set("item", &JsValue::from_f64(*item as f64));
            set("state", &JsValue::from_f64(*state as f64));
        }
        Event::ColdTiles { macro_position, subtype_id, layer_id, tic, tiles } => {
            set("kind", &JsValue::from_str("coldTiles"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("subtypeId", &JsValue::from_f64(*subtype_id as f64));
            set("layerId", &JsValue::from_f64(*layer_id as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            // 256 dense u16 kind_references, index = tile_reference; ship as a Uint16Array.
            let arr = js_sys::Uint16Array::new_with_length(tiles.len() as u32);
            arr.copy_from(tiles);
            set("tiles", &arr);
        }
        Event::ColdThings { macro_position, subtype_id, layer_id, tic, things } => {
            set("kind", &JsValue::from_str("coldThings"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("subtypeId", &JsValue::from_f64(*subtype_id as f64));
            set("layerId", &JsValue::from_f64(*layer_id as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            // Sparse u32 kind_pos_references; ship as a Uint32Array.
            let arr = js_sys::Uint32Array::new_with_length(things.len() as u32);
            arr.copy_from(things);
            set("things", &arr);
        }
        Event::ColdState { macro_position, entity_reference, position_reference, definition_reference, data, tic, removed } => {
            // TEMP (logs-drop drill): race-free receipt counter, readable from t=0.
            let _ = js_sys::eval("window.__csRecv=(window.__csRecv||0)+1");
            set("kind", &JsValue::from_str("coldState"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            set("positionReference", &JsValue::from_f64(*position_reference as f64));
            set("definitionReference", &JsValue::from_f64(*definition_reference as f64));
            set("data", &JsValue::from_f64(*data as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            set("removed", &JsValue::from_bool(*removed));
        }
        Event::ZoneClosed { macro_position } => {
            set("kind", &JsValue::from_str("zoneClosed"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
        }
        Event::Paused { paused } => {
            set("kind", &JsValue::from_str("paused"));
            set("paused", &JsValue::from_bool(*paused));
        }
        Event::TicAnchor { tic, wall_ms, tics_per_sec } => {
            set("kind", &JsValue::from_str("ticAnchor"));
            set("tic", &JsValue::from_f64(*tic as f64));
            set("wallMs", &JsValue::from_f64(*wall_ms));
            set("ticsPerSec", &JsValue::from_f64(*tics_per_sec));
        }
        Event::MoveIntent { macro_position, entity_reference, tile_x, tile_y, event_tic } => {
            set("kind", &JsValue::from_str("moveIntent"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            set("tileX", &JsValue::from_f64(*tile_x as f64));
            set("tileY", &JsValue::from_f64(*tile_y as f64));
            set("eventTic", &JsValue::from_f64(*event_tic as f64));
        }
        Event::QueueState { macro_position, entity_reference, event_tic, entries } => {
            set("kind", &JsValue::from_str("queueState"));
            set("macroPosition", &JsValue::from_f64(*macro_position as f64));
            set("entityReference", &JsValue::from_f64(*entity_reference as f64));
            set("eventTic", &JsValue::from_f64(*event_tic as f64));
            // Flat stride-4 entry words, verbatim (intent-queue-ui F1).
            let arr = js_sys::Uint32Array::new_with_length(entries.len() as u32);
            arr.copy_from(entries);
            set("entries", &arr);
        }
        Event::CallStats(stats) => {
            set("kind", &JsValue::from_str("callStats"));
            let arr = js_sys::Array::new();
            for s in stats {
                let o = js_sys::Object::new();
                let put = |key: &str, value: &JsValue| {
                    let _ = js_sys::Reflect::set(&o, &JsValue::from_str(key), value);
                };
                put("command", &JsValue::from_str(&s.command));
                put("requests", &JsValue::from_f64(s.requests as f64));
                put("ok", &JsValue::from_f64(s.ok as f64));
                put("err", &JsValue::from_f64(s.err as f64));
                put("tx", &JsValue::from_f64(s.tx as f64));
                put("rx", &JsValue::from_f64(s.rx as f64));
                arr.push(&o);
            }
            set("stats", &arr);
        }
        Event::ClockSync(s) => {
            // Fields line up with webgl's `ClockStats` so the sync HUD renders
            // them directly. Optionals become `null` until a real pong lands.
            let num = |v: f64| JsValue::from_f64(v);
            let opt_u = |v: Option<u64>| v.map_or(JsValue::NULL, |x| JsValue::from_f64(x as f64));
            let opt_i = |v: Option<i64>| v.map_or(JsValue::NULL, |x| JsValue::from_f64(x as f64));
            set("kind", &JsValue::from_str("clockSync"));
            set("serverNowMs", &num(s.server_now_ms as f64));
            set("synced", &JsValue::from_bool(s.synced));
            set("offsetMs", &num(s.offset_ms as f64));
            set("captures", &num(s.samples as f64));
            set("rttSamples", &num(s.samples as f64));
            set("rttMs", &opt_u(s.rtt_ms));
            set("bestRttMs", &opt_u(s.best_rtt_ms));
            set("bestOffsetMs", &opt_i(s.best_offset_ms));
            set("worstOffsetMs", &opt_i(s.worst_offset_ms));
        }
        Event::SubStats {
            open,
            total,
            tables,
        } => {
            set("kind", &JsValue::from_str("subStats"));
            set("open", &JsValue::from_f64(*open as f64));
            set("total", &JsValue::from_f64(*total as f64));
            let arr = js_sys::Array::new();
            for s in tables {
                let o = js_sys::Object::new();
                let put = |key: &str, value: &JsValue| {
                    let _ = js_sys::Reflect::set(&o, &JsValue::from_str(key), value);
                };
                put("table", &JsValue::from_str(&s.table));
                put("rows", &JsValue::from_f64(s.rows as f64));
                put("rx", &JsValue::from_f64(s.rx as f64));
                arr.push(&o);
            }
            set("tables", &arr);
        }
    }

    obj.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegates_to_core() {
        assert_eq!(greeting("webgl"), "hello world from resonantdust shared, webgl");
    }

    #[test]
    fn loads_content_and_maps_def_to_colour() {
        // Native exercise of the same Bundle the `Content` surface wraps.
        let toml = "[[tile]]\nname = \"grass\"\ntexture = \"white\"\ntint = \"#4b573e\"\n";
        let bundle = dsl::load(&[("tiles.toml".into(), toml.into())]).expect("load");
        let id = bundle.tile_def_id("grass").unwrap();
        assert_eq!(bundle.color_bg_for_def(id), Some(0x4b573e));
    }
}
