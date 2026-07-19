//! Texture serving — the gateway resolves a client STEM to a master albedo and
//! hands back the requested resolution, DERIVING the lower ones from the master on
//! demand. Art writes only masters (`bin/art`); the gate replicates each into the
//! sizes the client needs, so a re-mastered asset needs no offline pyramid — the
//! next request just re-derives.
//!
//! Two tiers today, both keyed by stem (`<category>/<kind>[/<facing>]`):
//!   • `master`  — the full-res master albedo, served verbatim.
//!   • `preview` — the master downscaled by [`PREVIEW_SCALE`] (half), the geo→real
//!                 placeholder. Disk sources cache the derived PNG under
//!                 `derived/preview/` (invalidated when the master is newer); R2
//!                 sources derive per request.
//!
//! Stem → master path is deterministic. The stem's leading segments ARE the
//! `<cat>/<kind>` directory names verbatim (a named subcategory/subkind is a dotted
//! segment kept as-is); an optional TRAILING `n`/`e`/`s` segment names the facing
//! (absent → south). In the group-less folder-per-variant layout
//! (docs/components/dev/textures/design/texture-paths.md): `linked/wall.smooth` →
//! `linked/wall.smooth/1.s.0/1/albedo.png` and `world/conifer/e` →
//! `world/conifer/1.e.0/1/albedo.png` — the canonical instance
//! `<id=1>.<dir>.<layer=0>/<variant=1>/albedo.png`. The per-instance variation picker
//! (the old `^r2`) is future work; one variation per kind for now.
//!
//! Paths are sanitised to relative, `..`-free forms before they touch disk or the
//! R2 keyspace, so a crafted stem can never escape the texture root.

use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::content::R2Config;

/// Preview downscale relative to the master (half — a 32px module becomes 16).
/// Drop toward 0.25 to quarter it; the gate is the one place that decides.
const PREVIEW_SCALE: f32 = 0.5;

/// The texture MAPS a leaf can hold, one PNG per map in the same variant leaf. `albedo`
/// (colour) is mandatory and the default; `normal`/`depth`/`emissive` are optional (a
/// missing one is a clean 404 the client falls back for). `packed` (per-material weight,
/// The `albedo` map is the residual reconstruction base (RGB); `layers` (RGB weights) adds the
/// material tints; `surface` (R=height, G=ao, B=coverage) is the one silhouette source. `bin/art
/// split_layers` emits `albedo.png` (residual) + `layers.png` and preserves the de-lit source as
/// the build-only `albedo_marigold.png` (NOT served). The set doubles as the leaf FILENAME
/// allowlist — a `map` outside it never touches disk (`master_map_rel` → None).
pub const MAPS: [&str; 6] = ["albedo", "normal", "depth", "emissive", "layers", "surface"];

/// Where the gateway reads master texture bytes from — the binary analogue of
/// [`crate::content::ContentSource`].
pub enum TextureSource {
    /// A local texture root (dev — the bind-mounted, READ-ONLY `textures/`).
    /// Masters live at `<root>/<cat>/<kind>/…` (group-less folder-per-variant leaves);
    /// derived previews cache under `<cache>/…`, a separate WRITABLE dir (the master
    /// mount is read-only, so the cache can't live beneath it).
    Disk { root: PathBuf, cache: PathBuf },
    /// The Cloudflare R2 asset bucket (deployed). Masters live under
    /// `<prefix>/textures/<cat>/<kind>/…` (group-less; R2 must be re-synced to this
    /// layout — docs "R2 step"), reusing the content route's R2 credentials.
    R2(R2Config),
}

/// One served asset: the raw bytes plus its `Content-Type` (PNG throughout).
pub struct ServedTexture {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

impl TextureSource {
    /// A short label for logs (never includes secrets).
    pub fn label(&self) -> String {
        match self {
            TextureSource::Disk { root, cache } => format!("disk:{} (cache {})", root.display(), cache.display()),
            TextureSource::R2(c) => format!("r2:{}/{}/textures", c.bucket, c.prefix),
        }
    }

    /// Resolve a stem+map to the master leaf rel, honouring BOTH the canonical numeric
    /// variant leaf (`<…>/1/<map>.<facing>.0.png`) and a NAMED-variant leaf
    /// (`<…>/<form>/<map>.<facing>.0.png`, files directly — a biome-tile form like
    /// `smooth/wall`). On disk we probe which exists (named first); R2 uses the canonical.
    fn resolve_rel(&self, stem: &str, map: &str) -> Option<PathBuf> {
        if !MAPS.contains(&map) {
            return None;
        }
        // Named-variant probe (disk only): <segs>/<map>.<facing>.0.png (the form IS the last seg).
        if let TextureSource::Disk { root, .. } = self {
            let (segs, facing) = stem_segs_facing(stem)?;
            let named: PathBuf = segs.iter().collect::<PathBuf>().join(format!("{map}.{facing}.0.png"));
            if root.join(&named).exists() {
                return Some(named);
            }
        }
        master_map_rel(stem, map) // canonical <segs>/1/<map>.<facing>.0.png
    }

    /// The full-res master albedo for `stem`, served verbatim. `Ok(None)` is a
    /// clean miss (404).
    pub async fn serve_master(&self, stem: &str) -> Result<Option<ServedTexture>, String> {
        let rel = self.resolve_rel(stem, "albedo").ok_or_else(|| format!("bad texture stem: {stem}"))?;
        Ok(self
            .read_master(&rel)
            .await?
            .map(|bytes| ServedTexture { bytes, content_type: "image/png" }))
    }

    /// The `meta.json` sidecar for `stem` (packed-channel tints + the shadow-cast silhouette outline),
    /// served on demand — it sits beside the maps in the resolved leaf, so resolve via `albedo` then
    /// swap the filename. `Ok(None)` when absent (a stem `bin/art` hasn't written meta for yet).
    pub async fn serve_meta(&self, stem: &str) -> Result<Option<ServedTexture>, String> {
        let rel = self.resolve_rel(stem, "albedo").ok_or_else(|| format!("bad texture stem: {stem}"))?;
        let meta_rel = rel.with_file_name("meta.json");
        Ok(self
            .read_master(&meta_rel)
            .await?
            .map(|bytes| ServedTexture { bytes, content_type: "application/json" }))
    }

    /// The half-res preview for `stem`, derived from the master. Disk sources cache
    /// the derived PNG (re-deriving when the master is newer); R2 derives fresh.
    /// `Ok(None)` when the master is absent.
    pub async fn serve_preview(&self, stem: &str) -> Result<Option<ServedTexture>, String> {
        let rel = self.resolve_rel(stem, "albedo").ok_or_else(|| format!("bad texture stem: {stem}"))?;

        // Disk: serve a fresh cached derivation, else re-derive + cache.
        if let TextureSource::Disk { root, cache } = self {
            let master = root.join(&rel);
            if !master.exists() {
                return Ok(None);
            }
            let cached = derived_preview_path(cache, stem);
            if is_fresh(&cached, &master) {
                if let Ok(bytes) = std::fs::read(&cached) {
                    return Ok(Some(ServedTexture { bytes, content_type: "image/png" }));
                }
            }
            let master_bytes = std::fs::read(&master).map_err(|e| format!("read master {}: {e}", master.display()))?;
            let small = downscale(&master_bytes, PREVIEW_SCALE)?;
            if let Some(parent) = cached.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cached, &small); // best-effort cache; serving still works if it fails
            return Ok(Some(ServedTexture { bytes: small, content_type: "image/png" }));
        }

        // R2: no local cache — derive from the fetched master each request.
        match self.read_master(&rel).await? {
            Some(master_bytes) => {
                let small = downscale(&master_bytes, PREVIEW_SCALE)?;
                Ok(Some(ServedTexture { bytes: small, content_type: "image/png" }))
            }
            None => Ok(None),
        }
    }

    /// One LOD of `stem`: the master downscaled so its SHORT axis is `size` px,
    /// CLAMPED to the master (a `size` at or above the native short axis serves the
    /// master verbatim — we never upscale). Disk sources cache the derived PNG under
    /// `derived/lod/<size>/…` (re-derived when the master is newer); R2 derives
    /// fresh. `Ok(None)` when the master is absent.
    pub async fn serve_lod(&self, stem: &str, size: u32, map: &str) -> Result<Option<ServedTexture>, String> {
        let rel = self.resolve_rel(stem, map).ok_or_else(|| format!("bad texture stem/map: {stem} {map}"))?;

        // Disk: serve a fresh cached derivation, else re-derive + cache.
        if let TextureSource::Disk { root, cache } = self {
            let master = root.join(&rel);
            if !master.exists() {
                return Ok(None);
            }
            let cached = derived_lod_path(cache, size, map, stem);
            if is_fresh(&cached, &master) {
                if let Ok(bytes) = std::fs::read(&cached) {
                    return Ok(Some(ServedTexture { bytes, content_type: "image/png" }));
                }
            }
            let master_bytes = std::fs::read(&master).map_err(|e| format!("read master {}: {e}", master.display()))?;
            let lod = downscale_to_short(&master_bytes, size)?;
            if let Some(parent) = cached.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cached, &lod); // best-effort cache
            return Ok(Some(ServedTexture { bytes: lod, content_type: "image/png" }));
        }

        // R2: no local cache — derive from the fetched master each request.
        match self.read_master(&rel).await? {
            Some(master_bytes) => {
                let lod = downscale_to_short(&master_bytes, size)?;
                Ok(Some(ServedTexture { bytes: lod, content_type: "image/png" }))
            }
            None => Ok(None),
        }
    }

    /// A cheap ETag for a stem — the MASTER's identity (both tiers derive from it),
    /// so a client's cached preview revalidates when the master is re-mastered.
    /// Disk: the master's `mtime-size`; R2: `None` (no cheap stat — R2 skips
    /// conditional revalidation for now). `None` too when the master is absent.
    pub fn etag(&self, stem: &str) -> Option<String> {
        let rel = self.resolve_rel(stem, "albedo")?;
        match self {
            TextureSource::Disk { root, .. } => disk_etag(&root.join(rel)),
            TextureSource::R2(_) => None,
        }
    }

    /// Read a master file's bytes at master-relative `rel`. `Ok(None)` is a miss.
    async fn read_master(&self, rel: &Path) -> Result<Option<Vec<u8>>, String> {
        match self {
            TextureSource::Disk { root, .. } => match std::fs::read(root.join(rel)) {
                Ok(b) => Ok(Some(b)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(format!("read master {}: {e}", root.join(rel).display())),
            },
            TextureSource::R2(cfg) => r2_get_bytes(cfg, rel).await,
        }
    }
}

/// The master albedo path for a stem, in the group-less folder-per-variant layout
/// (docs/components/dev/textures/design/texture-paths.md): `<cat>/<kind>/1.<facing>.0/1/albedo.png` — each stem
/// segment IS the directory name verbatim (`world/conifer` → `world/conifer/…`,
/// `linked/wall.smooth` → `linked/wall.smooth/…`). `None` if the stem escapes its
/// root or isn't `<category>/<kind>` shaped. The direction tokens a stem's trailing
/// segment may name — folded into the pose dir's `<dir>` field rather than becoming
/// its own directory: the `n`/`e`/`s` facings plus `l` for LINKED (autotile) kinds.
/// West is never a physical sprite (the client mirrors east), so it never lands on
/// disk. NOTE: the `master/` group prefix is gone on disk; R2 masters must be
/// re-synced to the same group-less layout before an R2 deploy (docs "R2 step").
const FACINGS: [&str; 4] = ["n", "e", "s", "l"];

// Parse a client stem into (path segments, facing). A trailing facing (`s`/`e`/`n`/`l`)
// folds out only when a `<cat>/<kind>` prefix precedes it (≥3 segs), so a 2-segment kind
// whose name happens to be a facing letter is never mistaken for one. Absent → south (`s`).
fn stem_segs_facing(stem: &str) -> Option<(Vec<String>, &'static str)> {
    let safe = sanitize(stem)?;
    let mut segs: Vec<String> = Vec::new();
    for c in safe.components() {
        match c {
            Component::Normal(seg) => segs.push(seg.to_str()?.to_string()),
            _ => return None,
        }
    }
    let mut facing = "s";
    if segs.len() >= 3 {
        if let Some(f) = FACINGS.iter().copied().find(|&f| f == segs.last().unwrap().as_str()) {
            facing = f;
            segs.pop();
        }
    }
    if segs.is_empty() {
        return None;
    }
    Some((segs, facing))
}

// The CANONICAL numeric-variant leaf: `<segs>/1/<map>.<facing>.0.png`. `map` must be in the
// allowlist so a crafted segment can never name an arbitrary file. Pure (no fs) — the
// fs-aware named-variant resolution is `TextureSource::resolve_rel`.
fn master_map_rel(stem: &str, map: &str) -> Option<PathBuf> {
    if !MAPS.contains(&map) {
        return None;
    }
    let (segs, facing) = stem_segs_facing(stem)?;
    let mut out: PathBuf = segs.iter().collect();
    out.push("1");
    out.push(format!("{map}.{facing}.0.png"));
    Some(out)
}

/// The cache path for a stem's derived preview: `<cache>/preview/<stem>.albedo.png`
/// (the tier subdir leaves room for other derived resolutions later).
fn derived_preview_path(cache: &Path, stem: &str) -> PathBuf {
    let mut p = cache.join("preview");
    p.push(format!("{stem}.albedo.png"));
    p
}

/// The cache path for a stem's derived LOD: `<cache>/lod/<size>/<stem>.<map>.png`.
fn derived_lod_path(cache: &Path, size: u32, map: &str, stem: &str) -> PathBuf {
    let mut p = cache.join("lod");
    p.push(size.to_string());
    p.push(format!("{stem}.{map}.png"));
    p
}

/// A weak-ish ETag for a master file: `"<mtime_secs>-<size>"` in hex, quoted. Cheap
/// (one stat, no read) and moves whenever the master is rewritten. `None` if the
/// file or its mtime can't be read.
fn disk_etag(master: &Path) -> Option<String> {
    let meta = std::fs::metadata(master).ok()?;
    let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(format!("\"{:x}-{:x}\"", mtime, meta.len()))
}

/// Whether `cache` exists and is at least as new as `master` (so re-deriving would
/// reproduce it). A missing / older cache, or unreadable mtimes, read as stale.
fn is_fresh(cache: &Path, master: &Path) -> bool {
    let (Ok(c), Ok(m)) = (std::fs::metadata(cache), std::fs::metadata(master)) else {
        return false;
    };
    match (c.modified(), m.modified()) {
        (Ok(cm), Ok(mm)) => cm >= mm,
        _ => false,
    }
}

/// Downscale a PNG by `scale` (both axes), re-encoding to PNG. Dimensions floor to
/// at least 1px so a tiny master still yields a valid image.
fn downscale(png: &[u8], scale: f32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(png).map_err(|e| format!("decode master: {e}"))?;
    let w = ((img.width() as f32 * scale).round() as u32).max(1);
    let h = ((img.height() as f32 * scale).round() as u32).max(1);
    let small = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
    let mut out = Cursor::new(Vec::new());
    small.write_to(&mut out, image::ImageFormat::Png).map_err(|e| format!("encode preview: {e}"))?;
    Ok(out.into_inner())
}

/// Downscale a PNG so its SHORT axis becomes `size` px, preserving aspect. CLAMPED:
/// a `size` at or above the native short axis returns the master bytes verbatim (we
/// never upscale — the client learns the ceiling from the returned dimensions).
fn downscale_to_short(png: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(png).map_err(|e| format!("decode master: {e}"))?;
    let native_short = img.width().min(img.height());
    if size == 0 || size >= native_short {
        return Ok(png.to_vec());
    }
    downscale(png, size as f32 / native_short as f32)
}

/// Collapse a stem to a relative, `..`-free `PathBuf`, or `None` if it tries to
/// escape (absolute, `..`, a Windows prefix). Only `Normal` segments survive.
fn sanitize(rel: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(seg) => out.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// GET one master's bytes from R2 at `<prefix>/textures/<rel>` via a
/// short-lived SigV4-presigned URL — the binary sibling of [`crate::content`]'s
/// `r2_get_text`. A 404 maps to `Ok(None)`; other failures are `Err`.
async fn r2_get_bytes(cfg: &R2Config, rel: &Path) -> Result<Option<Vec<u8>>, String> {
    use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

    let rel_str = rel.to_str().ok_or("non-utf8 texture path")?;
    let base = url::Url::parse(&cfg.endpoint).map_err(|e| format!("bad R2 endpoint: {e}"))?;
    let bucket = Bucket::new(base, UrlStyle::Path, cfg.bucket.clone(), cfg.region.clone())
        .map_err(|e| format!("R2 bucket: {e}"))?;
    let creds = Credentials::new(cfg.access_key.clone(), cfg.secret_key.clone());
    let key = format!("{}/textures/{}", cfg.prefix, rel_str);

    let action = bucket.get_object(Some(&creds), &key);
    let signed = action.sign(Duration::from_secs(60));
    let resp = reqwest::get(signed).await.map_err(|e| format!("R2 GET {key}: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let resp = resp.error_for_status().map_err(|e| format!("R2 GET {key}: {e}"))?;
    let bytes = resp.bytes().await.map_err(|e| format!("R2 GET {key} body: {e}"))?;
    Ok(Some(bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_resolves_to_canonical_master() {
        // no facing → south default, canonical leaf `1.s.0/1/albedo.png`; segments
        // verbatim (a named subkind like `wall.smooth` is kept as-is), group dropped
        assert_eq!(
            master_map_rel("linked/wall.smooth", "albedo"),
            Some(PathBuf::from("linked/wall.smooth/1.s.0/1/albedo.png")),
        );
        // the `map` names the leaf file — normal/depth resolve beside the albedo
        assert_eq!(
            master_map_rel("linked/wall.smooth", "normal"),
            Some(PathBuf::from("linked/wall.smooth/1.s.0/1/normal.png")),
        );
        assert_eq!(
            master_map_rel("world/conifer/e", "depth"),
            Some(PathBuf::from("world/conifer/1.e.0/1/depth.png")),
        );
        // a trailing facing segment folds into the `<dir>` field
        assert_eq!(
            master_map_rel("world/conifer/e", "albedo"),
            Some(PathBuf::from("world/conifer/1.e.0/1/albedo.png")),
        );
        assert_eq!(
            master_map_rel("world/conifer/n", "albedo"),
            Some(PathBuf::from("world/conifer/1.n.0/1/albedo.png")),
        );
        // `l` (linked/autotile) is a direction token too
        assert_eq!(
            master_map_rel("linked/wall.smooth/l", "albedo"),
            Some(PathBuf::from("linked/wall.smooth/1.l.0/1/albedo.png")),
        );
        // a non-facing 3rd segment stays a directory (facing → south)
        assert_eq!(
            master_map_rel("world/conifer/foo", "albedo"),
            Some(PathBuf::from("world/conifer/foo/1.s.0/1/albedo.png")),
        );
        // an unknown map is rejected (leaf-filename allowlist)
        assert_eq!(master_map_rel("linked/wall.smooth", "../etc"), None);
        assert_eq!(master_map_rel("linked/wall.smooth", "roughness"), None);
        // escapes are rejected
        assert_eq!(master_map_rel("../secret", "albedo"), None);
        assert_eq!(master_map_rel("linked/../../etc", "albedo"), None);
    }

    /// A 4×4 red PNG downscales to 2×2 and re-decodes cleanly at half size.
    #[test]
    fn downscale_halves_dimensions() {
        let src = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 30, 30, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(src).write_to(&mut buf, image::ImageFormat::Png).unwrap();

        let small = downscale(&buf.into_inner(), 0.5).unwrap();
        let decoded = image::load_from_memory(&small).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2, 2));
    }

    #[tokio::test]
    async fn disk_serves_master_derives_and_caches_preview() {
        let base = std::env::temp_dir().join(format!("gw-tex-derive-{}", std::process::id()));
        let root = base.join("textures");
        let cache = base.join("cache");
        let _ = std::fs::remove_dir_all(&base);
        // group-less canonical instance leaf: linked/wall.smooth/1.s.0/1/albedo.png
        let leaf = root.join("linked/wall.smooth/1.s.0/1");
        std::fs::create_dir_all(&leaf).unwrap();
        // a 4×4 master albedo
        let master = image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 180, 40, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(master).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        std::fs::write(leaf.join("albedo.png"), buf.into_inner()).unwrap();

        let src = TextureSource::Disk { root: root.clone(), cache: cache.clone() };

        // master served verbatim (4×4)
        let m = src.serve_master("linked/wall.smooth").await.unwrap().expect("master present");
        assert_eq!(image::load_from_memory(&m.bytes).unwrap().width(), 4);

        // preview derived to 2×2, and cached to the (separate, writable) cache dir
        let p = src.serve_preview("linked/wall.smooth").await.unwrap().expect("preview derived");
        assert_eq!(image::load_from_memory(&p.bytes).unwrap().width(), 2);
        assert!(derived_preview_path(&cache, "linked/wall.smooth").exists(), "preview cached");

        // a LOD downscales to the requested short axis (4×4 → 2×2) and caches it
        let l = src.serve_lod("linked/wall.smooth", 2, "albedo").await.unwrap().expect("lod derived");
        assert_eq!(image::load_from_memory(&l.bytes).unwrap().width(), 2);
        assert!(derived_lod_path(&cache, 2, "albedo", "linked/wall.smooth").exists(), "lod cached");
        // a LOD at/above the master's short axis is clamped to the master (no upscale)
        let big = src.serve_lod("linked/wall.smooth", 16, "albedo").await.unwrap().expect("lod clamped");
        assert_eq!(image::load_from_memory(&big.bytes).unwrap().width(), 4);
        // a map with no leaf on disk (no normal.png here) is a clean miss, not an error
        assert!(src.serve_lod("linked/wall.smooth", 2, "normal").await.unwrap().is_none());
        // an un-mastered stem is a clean miss on all tiers
        assert!(src.serve_master("linked/nope").await.unwrap().is_none());
        assert!(src.serve_preview("linked/nope").await.unwrap().is_none());
        assert!(src.serve_lod("linked/nope", 4, "albedo").await.unwrap().is_none());

        // a mastered stem has an ETag; an un-mastered one doesn't
        assert!(src.etag("linked/wall.smooth").is_some());
        assert!(src.etag("linked/nope").is_none());

        std::fs::remove_dir_all(&base).unwrap();
    }
}
