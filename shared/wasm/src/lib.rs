//! Resonant Dust (world) client-side wasm crate — hello-world scaffold.
//!
//! Compiled to a browser wasm bundle (see the `wasm` service in `compose.yml`)
//! and imported by the pixijs client. The server does NOT consume this crate —
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
// The DSL — re-exported so native rlib consumers reach it through this one crate
// too. The browser surface ([`Content`]) wraps it behind the `js` feature.
pub use resonantdust_dsl as dsl;

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

// ---------- content runtime (js feature) ----------
//
// The client's view of the DSL: load the fetched `.rd` corpus once, then answer
// the per-tile queries the renderer makes. `def_id` is what arrives in a zone's
// packed tile slots (`tileDef` off the codec); `Content` turns it back into the
// `visual.color.bg` the painter fills with. The server loads the SAME corpus from
// disk and derives the SAME ids, so a `def_id` means one tile on both sides.

/// A loaded content corpus — the client's handle to the DSL. Built from the
/// fetched `.rd` sources; queried per tile while painting a zone.
#[cfg(feature = "js")]
#[wasm_bindgen]
pub struct Content {
    bundle: dsl::Bundle,
}

#[cfg(feature = "js")]
#[wasm_bindgen]
impl Content {
    /// Load a corpus from parallel `names` / `sources` arrays — one entry per
    /// fetched `.rd` file (`names` are for error messages; pass the `:data`
    /// sources before the `:visual` ones, the order the server uses so ids
    /// agree). Throws a string of every parse error if the corpus is bad.
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
    #[wasm_bindgen(js_name = tileNames)]
    pub fn tile_names(&self) -> Vec<String> {
        self.bundle.tile_names().to_vec()
    }

    /// Expand a zone's packed tile slots into renderable prims — the painter's
    /// per-zone call. Runs the codec (cell → coords, slot → `def_id`) and the DSL
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
            // The DSL on_create resolves the tint + geo colour; unknown defs fall
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

    /// Per-kind emitted light, stride-8 (`[r, g, b, intensity, reach, radius, height, flags]`;
    /// flags bit 0 = cast_shadows, bit 1 = hot). `reach == 0` ⇒ the kind emits no light.
    #[wasm_bindgen(js_name = thingLight)]
    pub fn thing_light(&self) -> Vec<f64> {
        self.bundle.thing_light()
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

    /// Render data for one MOBILE entity (a pawn — e.g. a wolf) at `(macro_position,
    /// location)` of content `kind`: its world-tile position plus the kind's
    /// `tint`/`geo_color`, as `[tileX, tileY, tint, geoColor]`. The single-entity
    /// sibling of [`Content::zone_cold_prims`] — the host adds the sprite using this
    /// position + the kind's stem/layout (from `thing_texture_stems`/`thing_layout`) and a
    /// facing from the entity's rotation. Position comes straight from the cell, so a
    /// mover the bot walks cell-by-cell lands on the same grid the cold things use.
    #[wasm_bindgen(js_name = moverPrim)]
    pub fn mover_prim(&self, macro_position: u16, location: u8, kind: u16) -> Vec<f64> {
        use resonantdust_codec::object;
        let (origin_x, origin_y) = macro_origin(macro_position);
        // `location` is a `tile_reference` (`tile_x:4 | tile_y:4`) — decode with the
        // canonical high/low nibble split the cold things + ground use, NOT the
        // transposing legacy `packed::cell_x/cell_y`.
        let tile_x = origin_x + object::ref_hi(location) as i64;
        let tile_y = origin_y + object::ref_lo(location) as i64;
        let visual = self.bundle.visual_for_object(kind);
        let tint = visual.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF);
        let geo = visual.as_ref().map(|v| v.geo_color).unwrap_or(tint);
        vec![tile_x as f64, tile_y as f64, tint as f64, geo as f64]
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
// subscription engine, wrapping the `client` crate's `web` transport. pixijs
// constructs one with the gateway base + an event callback, then drives it with
// `login` / `setAnchor` — the same `Command` verbs the native headless driver
// sends. Every `Event` the engine emits is marshaled to a small tagged JS object
// and handed to the callback.

/// The pixijs-facing world client. Owns the session; events arrive on the
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
    #[wasm_bindgen(js_name = moveEntity)]
    pub fn move_entity(&self, entity: u32, tile_x: i32, tile_y: i32) {
        let _ = self.inner.move_entity(entity, tile_x, tile_y);
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
            set("kind", &JsValue::from_str("status"));
            set("message", &JsValue::from_str(message));
        }
        Event::StateObject {
            macro_position,
            entity_reference,
            definition_reference,
            tile_x,
            tile_y,
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
            set("facing", &JsValue::from_f64(*facing as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            set("removed", &JsValue::from_bool(*removed));
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
            // Fields line up with pixijs's `ClockStats` so the sync HUD renders
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
        assert_eq!(greeting("pixijs"), "hello world from resonantdust shared, pixijs");
    }

    #[test]
    fn loads_content_and_maps_def_to_colour() {
        // Native exercise of the same Bundle the `Content` surface wraps.
        let data = "<tile>\n  ::grass>\n    :data>\n      @define>\n        0 return\n";
        let visual = "<tile>\n  ::grass>\n    :visual>\n      @define>\n        #4b573e &visual.color.bg set\n        0 return\n";
        let bundle = dsl::load(&[
            ("data/tiles.rd".into(), data.into()),
            ("visual/tiles.rd".into(), visual.into()),
        ])
        .expect("load");
        let id = bundle.tile_def_id("grass").unwrap();
        assert_eq!(bundle.color_bg_for_def(id), Some(0x4b573e));
    }
}
