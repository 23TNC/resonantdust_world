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
// The bit-packing codec — the same `zone_id` / `valid_at` / thing-tile layouts
// the server uses, so the client encodes and decodes them with identical code.
pub use resonantdust_codec::packed;
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

// ---------- zone_id codec (js feature) ----------
//
// The client composes a `zone_id` to subscribe to a zone and decomposes the
// ids it receives. Thin wrappers over the shared [`packed`] functions the server
// also calls — same bit-layout on both sides, by construction.

/// Compose a `zone_id` from `region_x | region_y | surface | zone_x | zone_y`.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = packZoneId)]
pub fn pack_zone_id_js(region_x: u8, region_y: u8, surface: u8, zone_x: u8, zone_y: u8) -> u32 {
    packed::pack_zone_id(region_x, region_y, surface, zone_x, zone_y)
}

/// The `region_id` owning a `zone_id` (its shard-routing key).
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = regionOf)]
pub fn region_of_js(zone_id: u32) -> u32 {
    packed::region_of(zone_id)
}

/// The `region_x` byte of a `zone_id`.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = zoneRegionX)]
pub fn zone_region_x_js(zone_id: u32) -> u8 {
    packed::zone_region_x(zone_id)
}

/// The `region_y` byte of a `zone_id`.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = zoneRegionY)]
pub fn zone_region_y_js(zone_id: u32) -> u8 {
    packed::zone_region_y(zone_id)
}

/// The `surface` byte of a `zone_id`.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = zoneSurface)]
pub fn zone_surface_js(zone_id: u32) -> u8 {
    packed::zone_surface(zone_id)
}

/// The in-region `zone_x` nibble of a `zone_id`.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = zoneX)]
pub fn zone_x_js(zone_id: u32) -> u8 {
    packed::zone_x(zone_id)
}

/// The in-region `zone_y` nibble of a `zone_id`.
#[cfg(feature = "js")]
#[wasm_bindgen(js_name = zoneY)]
pub fn zone_y_js(zone_id: u32) -> u8 {
    packed::zone_y(zone_id)
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
    pub fn zone_tile_prims(&self, zone_id: u32, tiles: Vec<u16>) -> Vec<f64> {
        let (origin_x, origin_y) = zone_origin(zone_id);
        let mut out = Vec::new();
        for (i, &slot) in tiles.iter().enumerate() {
            let def_id = packed::tile_def(slot);
            if def_id == 0 {
                continue; // empty cell — no floor/wall
            }
            let location = i as u8;
            let tile_x = origin_x + packed::cell_x(location) as i64;
            let tile_y = origin_y + packed::cell_y(location) as i64;
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

    /// Every thing's sprite driving-axis footprint in `object_id` order, indexed by
    /// the `defId` a thing / free-thing prim carries. `0.0` means "unset" — the
    /// host applies its own default; the renderer treats it as pixels (the min axis)
    /// and derives the other axis from the texture aspect.
    #[wasm_bindgen(js_name = thingSizes)]
    pub fn thing_sizes(&self) -> Vec<f64> {
        self.bundle.thing_sizes()
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

    /// Render coords + colour for one loose thing (object-shard `free_things`) —
    /// the painter's per-thing call, the sub-tile sibling of [`zone_tile_prims`].
    /// Returns the same **stride-5** record `[x, y, tint, geoColor, defId]`, where
    /// `x`/`y` are **fractional global tile coordinates**: the tile origin plus the
    /// `offset`'s `x_off/y_off` nibbles as a 1/16-tile fraction
    /// (`resonantdust_codec::packed::pack_offset`). The host scales by the 64 px
    /// tile size like a tile prim, but draws a smaller sprite so a loose thing sits
    /// visibly within its cell. `id` is the thing def (an `object_id`), so its
    /// visual comes from the THING namespace — `defId` indexes [`thingTextureStems`].
    #[wasm_bindgen(js_name = freeThingPrim)]
    pub fn free_thing_prim(&self, zone_id: u32, location: u8, offset: u8, id: u16) -> Vec<f64> {
        let (origin_x, origin_y) = zone_origin(zone_id);
        let tile_x = origin_x + packed::cell_x(location) as i64;
        let tile_y = origin_y + packed::cell_y(location) as i64;
        let steps = packed::OFFSET_STEPS as f64;
        let x = tile_x as f64 + packed::offset_x(offset) as f64 / steps;
        let y = tile_y as f64 + packed::offset_y(offset) as f64 / steps;
        let visual = self.bundle.visual_for_object(id);
        let tint = visual.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF);
        let geo = visual.as_ref().map(|v| v.geo_color).unwrap_or(tint);
        vec![x, y, tint as f64, geo as f64, id as f64]
    }

    /// Expand a zone's packed cold things (`cold_zones.things`) into renderable
    /// prims — the thing-layer sibling of [`zone_tile_prims`]. Each entry is
    /// `x:4 | y:4 | rotation:2 | object_id:12` ([`packed::pack_thing`]); this runs
    /// the codec (→ in-zone cell) and the DSL (`object_id` → the thing's `:visual`
    /// [`VisualParts`]) for every entry, returning a flat **stride-6** array
    /// `[tileX, tileY, tint, geoColor, defId, rotation, …]` in *global tile coordinates*
    /// (`defId` indexing [`thingTextureStems`]; `rotation` the packed 0..3 facing the
    /// host resolves to a texture `<dir>` segment). The host draws a sprite smaller than
    /// a tile, centred in the cell and above the ground. Empty entries
    /// (`object_id == 0`) are skipped. One boundary crossing per zone.
    #[wasm_bindgen(js_name = zoneThingPrims)]
    pub fn zone_thing_prims(&self, zone_id: u32, things: Vec<u32>) -> Vec<f64> {
        let (origin_x, origin_y) = zone_origin(zone_id);
        let mut out = Vec::new();
        for &entry in &things {
            let object_id = packed::thing_object_id(entry);
            if object_id == 0 {
                continue; // empty entry — nothing here
            }
            let tile_x = origin_x + packed::thing_x(entry) as i64;
            let tile_y = origin_y + packed::thing_y(entry) as i64;
            let visual = self.bundle.visual_for_object(object_id);
            let tint = visual.as_ref().map(|v| v.tint).unwrap_or(0x00FF_FFFF);
            let geo = visual.as_ref().map(|v| v.geo_color).unwrap_or(tint);
            out.push(tile_x as f64);
            out.push(tile_y as f64);
            out.push(tint as f64);
            out.push(geo as f64);
            out.push(object_id as f64);
            out.push(packed::thing_rotation(entry) as f64);
        }
        out
    }
}

/// A zone's origin in **global tile coordinates**: which region, then which zone
/// within it, each scaled by the zone/region edge in tiles. The shared prefix of
/// every prim-expansion call ([`Content::zone_tile_prims`], `free_thing_prim`,
/// `zone_thing_prims`).
#[cfg(feature = "js")]
fn zone_origin(zone_id: u32) -> (i64, i64) {
    let dim = packed::ZONE_DIM as i64;
    let region_dim = packed::REGION_DIM as i64;
    let ox = (packed::zone_region_x(zone_id) as i64 * region_dim + packed::zone_x(zone_id) as i64) * dim;
    let oy = (packed::zone_region_y(zone_id) as i64 * region_dim + packed::zone_y(zone_id) as i64) * dim;
    (ox, oy)
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

    /// Add or move the viewport anchor `name` to global tile `(tile_x, tile_y)`
    /// on `surface`, with the four hysteresis-tier reaches in tiles. `soul` is `0`
    /// for a viewport. Idempotent — cheap to call every pan.
    #[wasm_bindgen(js_name = setAnchor)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_anchor(
        &self,
        name: String,
        tile_x: i32,
        tile_y: i32,
        surface: u8,
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
        let _ = self.inner.set_anchor(name, tile_x, tile_y, surface, radii, soul);
    }

    /// Remove the anchor `name`, closing any subscriptions only it held.
    #[wasm_bindgen(js_name = removeAnchor)]
    pub fn remove_anchor(&self, name: String) {
        let _ = self.inner.remove_anchor(name);
    }

    /// Move the controllable thing toward global tile `(tile_x, tile_y)` — the
    /// server pathfinds and commits the move. No-op before login / if disconnected.
    #[wasm_bindgen(js_name = moveTo)]
    pub fn move_to(&self, tile_x: i32, tile_y: i32) {
        let _ = self.inner.move_to(tile_x, tile_y);
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
/// range); a zone's packed tiles go through as a `Uint16Array`.
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
            data_shard,
            server_url,
        } => {
            set("kind", &JsValue::from_str("loggedIn"));
            set("playerId", &JsValue::from_f64(*player_id as f64));
            set("dataShard", &JsValue::from_f64(*data_shard as f64));
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
            zone_id,
            obj_type,
            object_id,
            tic,
            location,
            removed,
        } => {
            set("kind", &JsValue::from_str("stateObject"));
            set("zoneId", &JsValue::from_f64(*zone_id as f64));
            set("objType", &JsValue::from_f64(*obj_type as f64));
            // 48-bit object_id fits JS's 2^53 safe-integer range.
            set("objectId", &JsValue::from_f64(*object_id as f64));
            set("tic", &JsValue::from_f64(*tic as f64));
            set("location", &JsValue::from_f64(*location as f64));
            set("removed", &JsValue::from_bool(*removed));
        }
        Event::ZoneClosed { zone_id } => {
            set("kind", &JsValue::from_str("zoneClosed"));
            set("zoneId", &JsValue::from_f64(*zone_id as f64));
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
