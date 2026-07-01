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
    /// (`def_id` → `visual.color.bg`, i.e. the `:visual @on_create` tint) for
    /// every non-empty cell, returning a flat **stride-3** array
    /// `[tileX, tileY, tint, …]` in *global tile coordinates* (the host scales by
    /// the 64 px tile size and fills a white texture tinted `tint`). Empty cells
    /// (`def_id == 0`) are skipped. One wasm-boundary crossing per zone, not 256.
    #[wasm_bindgen(js_name = zoneTilePrims)]
    pub fn zone_tile_prims(&self, zone_id: u32, tiles: Vec<u16>) -> Vec<f64> {
        // Zone origin in global tile coordinates: which region, then which zone
        // within it, each scaled by the zone/region edge in tiles.
        let region_x = packed::zone_region_x(zone_id) as i64;
        let region_y = packed::zone_region_y(zone_id) as i64;
        let zone_x = packed::zone_x(zone_id) as i64;
        let zone_y = packed::zone_y(zone_id) as i64;
        let dim = packed::ZONE_DIM as i64;
        let region_dim = packed::REGION_DIM as i64;
        let origin_x = (region_x * region_dim + zone_x) * dim;
        let origin_y = (region_y * region_dim + zone_y) * dim;

        let mut out = Vec::new();
        for (i, &slot) in tiles.iter().enumerate() {
            let def_id = packed::tile_def(slot);
            if def_id == 0 {
                continue; // empty cell — no floor/wall
            }
            let location = i as u8;
            let tile_x = origin_x + packed::cell_x(location) as i64;
            let tile_y = origin_y + packed::cell_y(location) as i64;
            // The DSL on_create resolves the tint; unknown defs fall back to white.
            let tint = self.bundle.color_bg_for_def(def_id).unwrap_or(0x00FF_FFFF);
            out.push(tile_x as f64);
            out.push(tile_y as f64);
            out.push(tint as f64);
        }
        out
    }
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
        Event::ZoneTiles { zone_id, tiles } => {
            set("kind", &JsValue::from_str("zoneTiles"));
            set("zoneId", &JsValue::from_f64(*zone_id as f64));
            let arr = js_sys::Uint16Array::new_with_length(tiles.len() as u32);
            arr.copy_from(tiles);
            set("tiles", &arr);
        }
        Event::ZoneClosed { zone_id } => {
            set("kind", &JsValue::from_str("zoneClosed"));
            set("zoneId", &JsValue::from_f64(*zone_id as f64));
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
