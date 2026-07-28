//! The texture manifest — the client-authoritative index of what LODs exist, so the
//! client builds every LOD URL from it and never speculatively requests one that
//! isn't there (a 404 against R2 is a billable operation).
//!
//! Per stem: a content `hash` (cache-buster — a re-master changes it, so a stale
//! client URL 404s and triggers a manifest refetch + retry), the master's `maxSize`
//! (short axis — the LOD ceiling the client clamps to), and the set of
//! already-generated `lods` (grown as the server derives new LODs on demand).
//!
//! Disk-only for now: the manifest is scanned from the disk master tree at boot and
//! re-scanned by a poll. An R2 source yields an empty manifest — its manifest is
//! future work, generated offline by `bin/art` alongside the master upload.

use crate::lock::RwRecover;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::textures::TextureSource;

struct Entry {
    hash: String,
    max_size: u32,
    lods: BTreeSet<u32>,
    /// Which maps (albedo|normal|layers|surface|…) have a `<map>.png` leaf on disk. The
    /// client uses this to skip requesting a map that isn't there (no speculative 404).
    /// `albedo` is always present (it's what makes the stem a stem).
    maps: BTreeSet<String>,
    /// A LINKED (autotile) atlas holds a `cols × rows` cell grid in ONE master texture;
    /// the client samples a cell by UV rather than fetching a per-cell stem. Present only
    /// when the leaf carries an `atlas.json` sidecar (`bin/art`'s grid output); `None` for
    /// an ordinary single-image stem. `pad` is the normalized per-cell inset `[padU, padV]`
    /// (a fraction of the whole atlas) the client trims off each cell's UV rect so a
    /// downscale can't bleed a neighbour cell across a cell edge.
    grid: Option<[u32; 2]>,
    pad: Option<[f32; 2]>,
    /// The stem's frame SPAN in tiles, and the pow2 square it therefore packs into
    /// (`span * tile_px`). Read from the leaf's `meta.json`, where `bin/art leaf-span` caches it
    /// — the value is AUTHORED in the DSL corpus (`thing.span`) and only mirrored here, so this
    /// is a convenience for the packer, never a second place to change it. `None` when the leaf
    /// predates the stamp. Note this is the frame's extent, NOT the prim's `footprint`: a conifer
    /// occupies 1x1 tiles and spans 2, so sizing from the footprint would halve its square.
    span: Option<u32>,
    square: Option<u32>,
}

struct State {
    entries: BTreeMap<String, Entry>,
    version: u64,
}

pub struct TextureManifest {
    state: RwLock<State>,
}

impl TextureManifest {
    /// Scan the source for masters and build the initial manifest.
    pub fn build(source: &TextureSource) -> Self {
        let mut entries = BTreeMap::new();
        if let TextureSource::Disk { root, .. } = source {
            scan_masters(root, &mut entries);
        }
        let version = version_of(&entries);
        Self { state: RwLock::new(State { entries, version }) }
    }

    /// The `/textures-manifest` JSON body: `{ version, textures: { stem: { hash,
    /// maxSize, lods } } }`.
    pub fn payload_json(&self) -> String {
        let st = self.state.read_r();
        let textures: serde_json::Map<String, serde_json::Value> = st
            .entries
            .iter()
            .map(|(stem, e)| {
                let mut row = serde_json::json!({
                    "hash": e.hash,
                    "maxSize": e.max_size,
                    "lods": e.lods.iter().copied().collect::<Vec<u32>>(),
                    "maps": e.maps.iter().cloned().collect::<Vec<String>>(),
                });
                let obj = row.as_object_mut().unwrap();
                // The frame span in tiles + the pow2 square it packs into. Absent on a leaf that
                // has not been stamped, so the client keeps its dimension-based fallback.
                if let Some(span) = e.span {
                    obj.insert("span".into(), serde_json::json!(span));
                }
                if let Some(square) = e.square {
                    obj.insert("square".into(), serde_json::json!(square));
                }
                // A linked-atlas stem carries its cell grid + per-cell inset so the client
                // can sample a cell by UV; an ordinary stem omits both.
                if let (Some(grid), Some(pad)) = (e.grid, e.pad) {
                    obj.insert("grid".into(), serde_json::json!(grid));
                    obj.insert("pad".into(), serde_json::json!(pad));
                }
                (stem.clone(), row)
            })
            .collect();
        serde_json::json!({ "version": format!("{:016x}", st.version), "textures": textures }).to_string()
    }

    /// The `/textures-manifest-version` fingerprint (16 hex).
    pub fn version_hex(&self) -> String {
        format!("{:016x}", self.state.read_r().version)
    }

    /// The current hash + master short-axis for `stem`, if it has a master.
    pub fn lookup(&self, stem: &str) -> Option<(String, u32)> {
        let st = self.state.read_r();
        st.entries.get(stem).map(|e| (e.hash.clone(), e.max_size))
    }

    /// Record that `size` was generated for `stem`; bumps the version if it's new.
    pub fn note_generated(&self, stem: &str, size: u32) {
        let mut st = self.state.write_r();
        let inserted = st.entries.get_mut(stem).map(|e| e.lods.insert(size)).unwrap_or(false);
        if inserted {
            st.version = version_of(&st.entries);
        }
    }

    /// Re-scan the source, updating hashes/sizes and bumping the version on any
    /// change. Preserves the generated-LOD set for a master whose hash is unchanged
    /// (so a routine re-scan doesn't churn the version); a re-mastered stem resets
    /// its LODs. Returns whether anything changed.
    pub fn refresh(&self, source: &TextureSource) -> bool {
        let TextureSource::Disk { root, .. } = source else { return false };
        let mut fresh = BTreeMap::new();
        scan_masters(root, &mut fresh);

        let mut st = self.state.write_r();
        let mut changed = fresh.len() != st.entries.len();
        for (stem, e) in fresh.iter_mut() {
            match st.entries.get(stem) {
                // Unchanged leaf (every map file's mtime/size identical): keep its
                // generated-LOD set. Any map rewrite/add/remove moves the hash, so it
                // falls through to the re-fetch path below.
                Some(old) if old.hash == e.hash => {
                    e.lods = old.lods.clone();
                    if old.maps != e.maps {
                        changed = true;
                    }
                }
                _ => changed = true,
            }
        }
        if changed {
            let entries = std::mem::take(&mut fresh);
            st.version = version_of(&entries);
            st.entries = entries;
        }
        changed
    }
}

/// Poll the source every `secs` seconds, re-scanning for re-mastered assets. The
/// client notices via its own `/textures-manifest-version` poll. `secs == 0`
/// disables it (scan-once at boot).
pub fn spawn_poll(manifest: Arc<TextureManifest>, source: Arc<TextureSource>, secs: u64) {
    if secs == 0 {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(secs));
        tick.tick().await; // consume the immediate first tick
        loop {
            tick.tick().await;
            if manifest.refresh(&source) {
                tracing::info!(version = %manifest.version_hex(), "texture manifest changed");
            }
        }
    });
}

/// The direction tokens whose canonical master (leaf `1.<dir>.0/1/albedo.png`) is
/// scanned into its own stem: the n/e/s facings plus `l` for linked (autotile) kinds.
/// West is never on disk — the client mirrors east.
const FACINGS: [&str; 4] = ["n", "e", "s", "l"];

/// The legacy `<group>` dirs kept on disk for rollback (docs/components/dev/textures/design/texture-paths.md, Phase
/// 2) — skipped so the group-less scan never mistakes them for categories.
const LEGACY_GROUPS: [&str; 3] = ["master", "sprites", "templates"];

/// Walk the texture tree to any depth, filling `entries` with one per-facing stem for
/// every KIND dir — a dir holding the canonical instance leaf `1.<facing>.0/1/albedo.png`.
/// The stem is the dir's path relative to `tex_root` (dir names ARE the stem segments
/// verbatim) plus the facing, so BOTH the legacy 2-level `<cat>/<kind>` layout and the
/// object-model 4-level `<type>/<subtype>/<kind>/<subkind>` layout scan into their stems
/// with no depth assumption (docs/components/shared/codec/design/object-model.md §8, docs/components/dev/textures/design/texture-paths.md). Each facing
/// is its own texture (own bytes, hash, LOD cache), so it gets its own manifest row.
fn scan_masters(tex_root: &Path, entries: &mut BTreeMap<String, Entry>) {
    scan_dir(tex_root, tex_root, entries);
}

/// Recurse `dir`: register it as a kind if it holds the canonical leaf, otherwise descend.
fn scan_dir(tex_root: &Path, dir: &Path, entries: &mut BTreeMap<String, Entry>) {
    let Ok(kids) = std::fs::read_dir(dir) else { return };
    for kid in kids.flatten() {
        let path = kid.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = dir_name(&kid.file_name()) else { continue };
        // Skip the legacy `<group>` rollback dirs and any `1.<facing>.<layer>` leaf-id dir
        // (its contents are the variant leaves, handled by its parent kind).
        if LEGACY_GROUPS.contains(&name.as_str()) || name.starts_with("1.") {
            continue;
        }
        let Some(rel) = path.strip_prefix(tex_root).ok().and_then(|p| p.to_str()) else { continue };
        // A dir with the canonical leaf IS a kind; else it's an intermediate — descend.
        if !register_kind(&path, rel, entries) {
            scan_dir(tex_root, &path, entries);
        }
    }
}

/// Register a kind dir's per-facing stems (`<stem_prefix>/<facing>`) if it holds the
/// canonical instance leaf `1.<facing>.0/1/albedo.png`. Returns whether any facing matched.
fn register_kind(kind_path: &Path, stem_prefix: &str, entries: &mut BTreeMap<String, Entry>) -> bool {
    // Canonical numeric variant `1/` → the implicit stem `<prefix>/<facing>`.
    let mut found = register_variant(&kind_path.join("1"), stem_prefix, entries);
    // NAMED (non-numeric) variant folders → explicit stems `<prefix>/<form>/<facing>` — a
    // biome-tile form like `smooth/wall`. A numeric variant is the (future) per-instance
    // variation picker, not addressable by a stem yet, so it's skipped here.
    if let Ok(kids) = std::fs::read_dir(kind_path) {
        for kid in kids.flatten() {
            let p = kid.path();
            if !p.is_dir() {
                continue;
            }
            let Some(name) = dir_name(&kid.file_name()) else { continue };
            if name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            found |= register_variant(&p, &format!("{stem_prefix}/{name}"), entries);
        }
    }
    found
}

// Register one variant leaf dir (`<kind>/1` or `<kind>/<form>`): a stem per facing whose
// `albedo.<facing>.0.png` is present, with its sibling maps + optional linked-atlas grid.
fn register_variant(leaf: &Path, stem_prefix: &str, entries: &mut BTreeMap<String, Entry>) -> bool {
    let mut found = false;
    for facing in FACINGS {
        let master = leaf.join(format!("albedo.{facing}.0.png"));
        if !master.is_file() {
            continue;
        }
        let Ok((w, h)) = image::image_dimensions(&master) else { continue };
        // Which of the known maps have a leaf here for this facing (albedo always does).
        let maps: BTreeSet<String> = crate::textures::MAPS
            .iter()
            .filter(|m| leaf.join(format!("{m}.{facing}.0.png")).is_file())
            .map(|m| m.to_string())
            .collect();
        // Hash ALL present map files (+ the `atlas.json` sidecar) so re-mastering any of them
        // moves the cache-buster hash.
        let Some(hash) = leaf_hash(leaf, &maps, facing) else { continue };
        // A `bin/art` linked atlas drops an `atlas.json` beside its maps: the cell grid + inset.
        let (span, square) = read_span_meta(leaf);
        let (grid, pad) = match read_atlas_meta(leaf) {
            Some((g, p)) => (Some(g), Some(p)),
            None => (None, None),
        };
        entries.insert(
            format!("{stem_prefix}/{facing}"),
            Entry { hash, max_size: w.min(h), lods: BTreeSet::new(), maps, grid, pad, span, square },
        );
        found = true;
    }
    found
}

fn dir_name(name: &std::ffi::OsStr) -> Option<String> {
    name.to_str().map(|s| s.to_string())
}

/// A URL-safe content hash for a leaf: FNV-1a over each present map file's `mtime-size`
/// (sorted by map name for determinism). Moves on any map's rewrite — a re-mastered
/// `albedo` OR `layers` OR `surface` all bust it, so the client's hash-addressed
/// caches (HTTP + IndexedDB) never serve stale map bytes.
fn leaf_hash(leaf: &Path, maps: &BTreeSet<String>, facing: &str) -> Option<String> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for map in maps {
        let meta = std::fs::metadata(leaf.join(format!("{map}.{facing}.0.png"))).ok()?;
        let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
        for b in format!("{map}:{mtime:x}-{:x};", meta.len()).bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    // Fold the SIDECARS too, so re-authoring metadata busts the hash even though neither is one of
    // the served `<map>.png` files. `atlas.json` carries the cell grid + inset; `meta.json` carries
    // the span/square the packer sizes from (and the tints and outline). Without meta.json here, a
    // re-stamped span would never reach a client that already has the stem cached.
    for sidecar in ["atlas.json", "meta.json"] {
        let Ok(meta) = std::fs::metadata(leaf.join(sidecar)) else { continue };
        if let Some(mtime) = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
            for b in format!("{sidecar}:{:x}-{:x};", mtime.as_secs(), meta.len()).bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    Some(format!("{h:x}"))
}

/// Read `span` + `square` from a leaf's `meta.json` (stamped by `bin/art leaf-span`). Both absent
/// is normal for a leaf that has not been re-mastered since the stamp landed, so this returns
/// `(None, None)` rather than failing — the client falls back to the texture's own dimensions.
fn read_span_meta(leaf: &Path) -> (Option<u32>, Option<u32>) {
    let Ok(text) = std::fs::read_to_string(leaf.join("meta.json")) else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, None);
    };
    (
        v.get("span").and_then(|x| x.as_u64()).map(|x| x as u32),
        v.get("square").and_then(|x| x.as_u64()).map(|x| x as u32),
    )
}

/// Parse a leaf's `atlas.json` (a `bin/art` linked-atlas sidecar) into `([cols, rows],
/// [padU, padV])`. `None` when the file is absent or malformed — the stem is then treated
/// as an ordinary single-image texture. Parsed via `serde_json::Value` (no derive dep).
fn read_atlas_meta(leaf: &Path) -> Option<([u32; 2], [f32; 2])> {
    let text = std::fs::read_to_string(leaf.join("atlas.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let cols = v.get("cols")?.as_u64()? as u32;
    let rows = v.get("rows")?.as_u64()? as u32;
    let pad_u = v.get("padU")?.as_f64()? as f32;
    let pad_v = v.get("padV")?.as_f64()? as f32;
    Some(([cols, rows], [pad_u, pad_v]))
}

/// FNV-1a over the sorted (stem, hash, lods, maps) — moves on a re-master, a new LOD, or a
/// newly-added map (e.g. a `normal.png` dropped beside an existing albedo).
fn version_of(entries: &BTreeMap<String, Entry>) -> u64 {
    fn feed(h: &mut u64, bytes: &[u8]) {
        for &b in bytes {
            *h ^= b as u64;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (stem, e) in entries {
        feed(&mut h, stem.as_bytes());
        feed(&mut h, e.hash.as_bytes());
        for s in &e.lods {
            feed(&mut h, &s.to_le_bytes());
        }
        for m in &e.maps {
            feed(&mut h, m.as_bytes());
        }
        // Fold the linked-atlas grid + inset so authoring/changing it bumps the version
        // (the client refetches the manifest and re-derives its cell UVs).
        if let Some(g) = e.grid {
            feed(&mut h, &g[0].to_le_bytes());
            feed(&mut h, &g[1].to_le_bytes());
        }
        if let Some(p) = e.pad {
            feed(&mut h, &p[0].to_le_bytes());
            feed(&mut h, &p[1].to_le_bytes());
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Write a canonical variant leaf: `<cat>/<kind>/<variant>/albedo.<dir>.<part>.png`.
    ///
    /// This helper used to write the SUPERSEDED `<cat>/<kind>/1.s.0/1/albedo.png` shape — no
    /// facing/part in the filename, an `<id>` segment that no longer exists. The scanner needs
    /// `albedo.<facing>.<part>.png`, so it matched nothing and every assertion below ran against
    /// an EMPTY manifest; the test had been passing no signal at all (stream I11). Returns the
    /// leaf so callers can drop siblings into it.
    fn write_master(root: &Path, cat: &str, kind: &str, w: u32, h: u32) -> std::path::PathBuf {
        let dir = root.join(cat).join(kind).join("1");
        std::fs::create_dir_all(&dir).unwrap();
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        std::fs::write(dir.join("albedo.s.0.png"), buf.into_inner()).unwrap();
        dir
    }

    #[test]
    fn surfaces_span_and_square_from_the_leaf_meta_json() {
        let base = std::env::temp_dir().join(format!("gw-span-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let leaf = write_master(&base, "biome-thing", "conifer", 256, 256);

        // Unstamped: the keys must be ABSENT, not defaulted — a leaf that predates the stamp
        // must not claim a span of 0 or 1, which would size it wrong rather than fall back.
        let m = TextureManifest::build(&TextureSource::Disk { root: base.clone(), cache: base.join("cache") });
        let before = m.payload_json();
        assert!(!before.contains("\"span\""), "unstamped leaf must not carry a span");

        // Stamped, as `bin/art leaf-span` writes it: span 2 tiles -> a 256 square at 128px tiles.
        // The conifer is the real case — footprint 1x1 but span 2, so a footprint-derived square
        // would be 128 and wrong.
        std::fs::write(leaf.join("meta.json"),
            r#"{"channel_tints":[[1,2,3]],"span":2,"square":256,"tile_px":128}"#).unwrap();
        let m2 = TextureManifest::build(&TextureSource::Disk { root: base.clone(), cache: base.join("cache") });
        let after = m2.payload_json();
        assert!(after.contains("\"span\":2"), "span surfaces: {after}");
        assert!(after.contains("\"square\":256"), "square surfaces: {after}");

        // meta.json is folded into the leaf hash, so a re-stamp reaches a client that already
        // has the stem cached.
        let (h1, _) = m.lookup("biome-thing/conifer/s").expect("stem");
        let (h2, _) = m2.lookup("biome-thing/conifer/s").expect("stem");
        assert_ne!(h1, h2, "stamping meta.json must bust the leaf hash");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scans_stems_records_max_short_axis_and_notes_generated() {
        let base = std::env::temp_dir().join(format!("gw-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let leaf = write_master(&base, "linked", "wall.smooth", 64, 128); // short axis = 64

        let src = TextureSource::Disk { root: base.clone(), cache: base.join("cache") };
        let m = TextureManifest::build(&src);

        // the south master scans into a facing-suffixed stem
        let (hash, max) = m.lookup("linked/wall.smooth/s").expect("stem scanned");
        assert_eq!(max, 64, "max size is the short axis");
        assert!(!hash.is_empty());
        assert!(m.lookup("linked/wall.smooth").is_none(), "bare stem is not indexed");
        assert!(m.lookup("linked/nope/s").is_none());

        // an albedo-only leaf lists just albedo; a normal dropped beside it bumps the
        // version and shows up in the maps set (and moves the leaf hash, since the hash now
        // covers every present map file).
        assert!(m.payload_json().contains("\"maps\":[\"albedo\"]"));
        let v_maps = m.version_hex();
        let img = image::RgbaImage::from_pixel(64, 128, image::Rgba([128, 128, 255, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        std::fs::write(leaf.join("normal.s.0.png"), buf.into_inner()).unwrap();
        assert!(m.refresh(&src), "a new map beside an unchanged albedo is a change");
        assert_ne!(m.version_hex(), v_maps, "version bumps when a map is added");
        assert!(m.payload_json().contains("\"maps\":[\"albedo\",\"normal\"]"));

        // note_generated adds the LOD and bumps the version
        let v0 = m.version_hex();
        m.note_generated("linked/wall.smooth/s", 32);
        assert_ne!(m.version_hex(), v0, "version bumps when a LOD is generated");
        assert!(m.payload_json().contains("\"lods\":[32]"));
        // a repeat is a no-op (no churn)
        let v1 = m.version_hex();
        m.note_generated("linked/wall.smooth/s", 32);
        assert_eq!(m.version_hex(), v1);

        // A linked-atlas sidecar (bin/art's grid output) beside the maps adds `grid` + `pad`
        // to the stem's manifest row and bumps the version; absent, the row omits both.
        // (Pad values are exact powers of two so they serialize without float noise.)
        assert!(!m.payload_json().contains("\"grid\""), "no atlas.json → no grid field");
        let v_pre_atlas = m.version_hex();
        std::fs::write(leaf.join("atlas.json"), br#"{"cols":4,"rows":4,"padU":0.125,"padV":0.0625}"#).unwrap();
        assert!(m.refresh(&src), "an atlas.json sidecar is a change");
        assert_ne!(m.version_hex(), v_pre_atlas, "version bumps when the atlas grid lands");
        let payload = m.payload_json();
        assert!(payload.contains("\"grid\":[4,4]"), "grid round-trips: {payload}");
        assert!(payload.contains("\"pad\":[0.125,0.0625]"), "pad round-trips: {payload}");

        std::fs::remove_dir_all(&base).unwrap();
    }
}
