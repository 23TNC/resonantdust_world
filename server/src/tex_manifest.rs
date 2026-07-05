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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::textures::TextureSource;

struct Entry {
    hash: String,
    max_size: u32,
    lods: BTreeSet<u32>,
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
            scan_masters(&root.join("master"), &mut entries);
        }
        let version = version_of(&entries);
        Self { state: RwLock::new(State { entries, version }) }
    }

    /// The `/textures-manifest` JSON body: `{ version, textures: { stem: { hash,
    /// maxSize, lods } } }`.
    pub fn payload_json(&self) -> String {
        let st = self.state.read().unwrap();
        let textures: serde_json::Map<String, serde_json::Value> = st
            .entries
            .iter()
            .map(|(stem, e)| {
                (
                    stem.clone(),
                    serde_json::json!({
                        "hash": e.hash,
                        "maxSize": e.max_size,
                        "lods": e.lods.iter().copied().collect::<Vec<u32>>(),
                    }),
                )
            })
            .collect();
        serde_json::json!({ "version": format!("{:016x}", st.version), "textures": textures }).to_string()
    }

    /// The `/textures-manifest-version` fingerprint (16 hex).
    pub fn version_hex(&self) -> String {
        format!("{:016x}", self.state.read().unwrap().version)
    }

    /// The current hash + master short-axis for `stem`, if it has a master.
    pub fn lookup(&self, stem: &str) -> Option<(String, u32)> {
        let st = self.state.read().unwrap();
        st.entries.get(stem).map(|e| (e.hash.clone(), e.max_size))
    }

    /// Record that `size` was generated for `stem`; bumps the version if it's new.
    pub fn note_generated(&self, stem: &str, size: u32) {
        let mut st = self.state.write().unwrap();
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
        scan_masters(&root.join("master"), &mut fresh);

        let mut st = self.state.write().unwrap();
        let mut changed = fresh.len() != st.entries.len();
        for (stem, e) in fresh.iter_mut() {
            match st.entries.get(stem) {
                Some(old) if old.hash == e.hash => e.lods = old.lods.clone(), // unchanged master
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

/// The direction tokens whose canonical master (`1.<dir>.0.1.albedo.png`) is scanned
/// into its own stem: the n/e/s facings plus `l` for linked (autotile) kinds. West
/// is never on disk — the client mirrors east.
const FACINGS: [&str; 4] = ["n", "e", "s", "l"];

/// Walk `master/<cat>/<kind>/1.<facing>.0.1.albedo.png`, filling `entries` with one
/// per-facing stem (`<cat>/<kind>/<facing>` — the dir names ARE the stem segments
/// verbatim, named subkinds included) → hash + master short axis. Each facing is its
/// own texture (own bytes, hash, LOD cache), so it gets its own manifest row.
fn scan_masters(master_root: &Path, entries: &mut BTreeMap<String, Entry>) {
    let Ok(cats) = std::fs::read_dir(master_root) else { return };
    for cat in cats.flatten() {
        let Ok(kinds) = std::fs::read_dir(cat.path()) else { continue };
        for kind in kinds.flatten() {
            let (Some(cat_name), Some(kind_name)) = (dir_name(&cat.file_name()), dir_name(&kind.file_name())) else {
                continue;
            };
            for facing in FACINGS {
                let master = kind.path().join(format!("1.{facing}.0.1.albedo.png"));
                if !master.is_file() {
                    continue;
                }
                let Some(hash) = master_hash(&master) else { continue };
                let Ok((w, h)) = image::image_dimensions(&master) else { continue };
                let stem = format!("{cat_name}/{kind_name}/{facing}");
                entries.insert(stem, Entry { hash, max_size: w.min(h), lods: BTreeSet::new() });
            }
        }
    }
}

fn dir_name(name: &std::ffi::OsStr) -> Option<String> {
    name.to_str().map(|s| s.to_string())
}

/// A URL-safe content hash for a master: hex `mtime-size` (moves on any rewrite).
fn master_hash(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{:x}-{:x}", mtime, meta.len()))
}

/// FNV-1a over the sorted (stem, hash, lods) — moves on a re-master or a new LOD.
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
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn write_master(root: &Path, cat: &str, kind: &str, w: u32, h: u32) {
        let dir = root.join("master").join(cat).join(kind);
        std::fs::create_dir_all(&dir).unwrap();
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        std::fs::write(dir.join("1.s.0.1.albedo.png"), buf.into_inner()).unwrap();
    }

    #[test]
    fn scans_stems_records_max_short_axis_and_notes_generated() {
        let base = std::env::temp_dir().join(format!("gw-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        write_master(&base, "linked", "wall.smooth", 64, 128); // short axis = 64; named subkind

        let src = TextureSource::Disk { root: base.clone(), cache: base.join("cache") };
        let m = TextureManifest::build(&src);

        // the south master scans into a facing-suffixed stem
        let (hash, max) = m.lookup("linked/wall.smooth/s").expect("stem scanned");
        assert_eq!(max, 64, "max size is the short axis");
        assert!(!hash.is_empty());
        assert!(m.lookup("linked/wall.smooth").is_none(), "bare stem is not indexed");
        assert!(m.lookup("linked/nope/s").is_none());

        // note_generated adds the LOD and bumps the version
        let v0 = m.version_hex();
        m.note_generated("linked/wall.smooth/s", 32);
        assert_ne!(m.version_hex(), v0, "version bumps when a LOD is generated");
        assert!(m.payload_json().contains("\"lods\":[32]"));
        // a repeat is a no-op (no churn)
        let v1 = m.version_hex();
        m.note_generated("linked/wall.smooth/s", 32);
        assert_eq!(m.version_hex(), v1);

        std::fs::remove_dir_all(&base).unwrap();
    }
}
