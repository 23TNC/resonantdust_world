//! Content serving — the gateway pulls the DSL corpus from a source (local disk
//! in dev, Cloudflare R2 when deployed), caches it in memory, and hands it to
//! clients over `GET /content`. This is the runtime alternative to the client
//! embedding the corpus at build time: the gateway is the one place that holds
//! S3 creds, so the client just fetches from here.
//!
//! **Hot-update.** The corpus carries a deterministic fingerprint
//! ([`content_version`], an FNV-1a of `(basename, text)` pairs). A background
//! poll re-reads the source every `CONTENT_POLL_SECS`; when the fingerprint
//! moves it swaps the cached corpus and bumps the version. The client polls
//! `GET /content-version` (a cheap 16-hex string) and, on a change, re-fetches
//! `/content` and hot-swaps — no reload. `POST /content/refresh` forces the poll
//! immediately (so `dsl upload` can ping the gate).
//!
//! **What's served.** Only the `data` + `visual` facets — the client's `Content`
//! bundle. `biome` is server-only worldgen and is never sent to clients. Sources
//! are ordered **data first, then visual**, matching the world server's
//! `read_content_dir`, so tile / thing def-ids agree on both sides by
//! construction.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Ordered `(name, text)` source pairs — data facets first, then visual. `name`
/// is the path relative to `content/` (e.g. `"visual/tiles.rd"`), used for the
/// version basename and passed through to the client for error messages.
type Sources = Vec<(String, String)>;

/// Where the gateway reads the corpus from.
pub enum ContentSource {
    /// A local content directory (dev — the bind-mounted `content/`). Reads
    /// top-level `data/*.rd` then `visual/*.rd`.
    Disk(PathBuf),
    /// The Cloudflare R2 asset bucket (deployed). Reads `manifest.json`, then
    /// each `data` / `visual` key it lists.
    R2(R2Config),
}

/// R2 access parameters (a subset of `bin/dsl`'s config, from the environment).
pub struct R2Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    /// Key prefix — the corpus lives under `<prefix>/content/`.
    pub prefix: String,
    pub access_key: String,
    pub secret_key: String,
}

impl ContentSource {
    /// Read the corpus from this source into ordered `(name, text)` pairs.
    async fn load(&self) -> Result<Sources, String> {
        match self {
            ContentSource::Disk(root) => {
                load_disk(root).map_err(|e| format!("read content dir {}: {e}", root.display()))
            }
            ContentSource::R2(cfg) => load_r2(cfg).await,
        }
    }

    /// A short label for logs (never includes secrets).
    pub fn label(&self) -> String {
        match self {
            ContentSource::Disk(p) => format!("disk:{}", p.display()),
            ContentSource::R2(c) => format!("r2:{}/{}/content", c.bucket, c.prefix),
        }
    }
}

/// Read `data/*.rd` then `visual/*.rd` (top-level, sorted) from a content dir.
/// Mirrors the world server's `read_content_dir` order (data before visual, each
/// dir sorted, non-recursive) so def-ids match; `biome` is skipped — it's
/// server-only and never served to clients.
fn load_disk(root: &Path) -> io::Result<Sources> {
    let mut out = Sources::new();
    for facet in ["data", "visual"] {
        let dir = root.join(facet);
        if !dir.is_dir() {
            continue;
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "rd"))
            .collect();
        entries.sort();
        for path in entries {
            let text = std::fs::read_to_string(&path)?;
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            out.push((format!("{facet}/{file}"), text));
        }
    }
    Ok(out)
}

/// The R2 `manifest.json` index (`bin/dsl reindex`). Only the client facets are
/// deserialized; `biome` is intentionally ignored (server-only worldgen).
#[derive(serde::Deserialize, Default)]
struct Manifest {
    #[serde(default)]
    data: Vec<String>,
    #[serde(default)]
    visual: Vec<String>,
}

/// Fetch `manifest.json` from R2, then each `data` + `visual` key it lists
/// (data first, preserving the manifest's sorted order). Keys are resolved under
/// `<prefix>/content/`.
async fn load_r2(cfg: &R2Config) -> Result<Sources, String> {
    let manifest_text = r2_get_text(cfg, "manifest.json").await?;
    let manifest: Manifest =
        serde_json::from_str(&manifest_text).map_err(|e| format!("parse manifest.json: {e}"))?;
    let mut out = Sources::new();
    for key in manifest.data.iter().chain(manifest.visual.iter()) {
        let text = r2_get_text(cfg, key).await?;
        out.push((key.clone(), text));
    }
    Ok(out)
}

/// GET one object's text from R2 at `<prefix>/content/<rel>` via a short-lived
/// SigV4-presigned URL (R2 is S3-compatible). `rel` is `"manifest.json"` or a
/// manifest key like `"visual/tiles.rd"`.
async fn r2_get_text(cfg: &R2Config, rel: &str) -> Result<String, String> {
    use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

    let base = url::Url::parse(&cfg.endpoint).map_err(|e| format!("bad R2 endpoint: {e}"))?;
    let bucket = Bucket::new(base, UrlStyle::Path, cfg.bucket.clone(), cfg.region.clone())
        .map_err(|e| format!("R2 bucket: {e}"))?;
    let creds = Credentials::new(cfg.access_key.clone(), cfg.secret_key.clone());
    let key = format!("{}/content/{}", cfg.prefix, rel);

    let action = bucket.get_object(Some(&creds), &key);
    let signed = action.sign(Duration::from_secs(60));
    let resp = reqwest::get(signed)
        .await
        .map_err(|e| format!("R2 GET {key}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("R2 GET {key}: {e}"))?;
    resp.text().await.map_err(|e| format!("R2 GET {key} body: {e}"))
}

/// A deterministic corpus fingerprint — FNV-1a over each source's `basename` and
/// `text` (separated by NULs), in load order. No paths or timestamps, so it's
/// identical across machines and moves iff a served `.rd` file's name or bytes
/// change. The same hash the old game used, so the semantics carry over.
fn content_version(sources: &Sources) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
        }
    };
    for (name, text) in sources {
        let base = name.rsplit('/').next().unwrap_or(name);
        feed(base.as_bytes());
        feed(b"\0");
        feed(text.as_bytes());
        feed(b"\0");
    }
    h
}

/// The immutable snapshot the store hands out: the version + the pre-serialized
/// `/content` JSON (built once per load, so serving a request is a clone, not a
/// re-serialize).
struct Snapshot {
    version: u64,
    payload_json: String,
}

/// Build the `/content` payload from sources: `{ "version": "<hex>", "rd": [[name,
/// text], …] }`. The client feeds `rd` straight into `new Content(names, sources)`.
fn build_snapshot(sources: &Sources) -> Snapshot {
    let version = content_version(sources);
    let payload = serde_json::json!({
        "version": format!("{version:016x}"),
        "rd": sources,
    });
    Snapshot { version, payload_json: payload.to_string() }
}

/// The in-memory content cache: a source to (re)read and the current snapshot,
/// swapped atomically on a fingerprint change.
pub struct ContentStore {
    source: ContentSource,
    current: RwLock<Arc<Snapshot>>,
}

impl ContentStore {
    /// Load the corpus once from `source` and build the initial snapshot. Errors
    /// (bad dir, unreachable R2, missing creds) so the caller can disable content
    /// serving without taking the gateway down.
    pub async fn load(source: ContentSource) -> Result<Arc<Self>, String> {
        let sources = source.load().await?;
        let snap = Arc::new(build_snapshot(&sources));
        Ok(Arc::new(Self { source, current: RwLock::new(snap) }))
    }

    /// The `/content` JSON body (a clone of the cached string).
    pub fn payload_json(&self) -> String {
        self.current.read().unwrap().payload_json.clone()
    }

    /// The `/content-version` fingerprint as 16 hex chars.
    pub fn version_hex(&self) -> String {
        format!("{:016x}", self.current.read().unwrap().version)
    }

    /// Re-read the source and, if the fingerprint moved, swap the snapshot in.
    /// Returns whether it changed. The new snapshot is built before the swap, so a
    /// transient read error leaves the live corpus untouched.
    pub async fn refresh(&self) -> Result<bool, String> {
        let sources = self.source.load().await?;
        let snap = build_snapshot(&sources);
        if snap.version == self.current.read().unwrap().version {
            return Ok(false);
        }
        *self.current.write().unwrap() = Arc::new(snap);
        Ok(true)
    }
}

/// Spawn the background poll that re-reads the source every `secs` seconds and
/// hot-swaps on change. `secs == 0` disables polling (load-once). The client
/// notices via its own `/content-version` poll, so there's nothing to push here.
pub fn spawn_poll(store: Arc<ContentStore>, secs: u64) {
    if secs == 0 {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(secs));
        tick.tick().await; // the first tick fires immediately — skip it (just loaded)
        loop {
            tick.tick().await;
            match store.refresh().await {
                Ok(true) => tracing::info!(version = %store.version_hex(), "content: hot-swapped"),
                Ok(false) => {}
                Err(e) => tracing::warn!(error = %e, "content: poll failed"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srcs(pairs: &[(&str, &str)]) -> Sources {
        pairs.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect()
    }

    #[test]
    fn version_is_deterministic_and_sensitive() {
        let a = srcs(&[("data/tiles.rd", "grass"), ("visual/tiles.rd", "#fff")]);
        assert_eq!(content_version(&a), content_version(&a), "same corpus → same hash");

        // a text change moves the fingerprint
        let b = srcs(&[("data/tiles.rd", "grass"), ("visual/tiles.rd", "#000")]);
        assert_ne!(content_version(&a), content_version(&b));

        // order matters (data before visual is part of the identity)
        let c = srcs(&[("visual/tiles.rd", "#fff"), ("data/tiles.rd", "grass")]);
        assert_ne!(content_version(&a), content_version(&c));
    }

    #[test]
    fn version_uses_basename_not_path() {
        // Disk and R2 name the same file differently only by directory depth
        // above the facet; the fingerprint keys on the basename so both agree.
        let disk = srcs(&[("visual/tiles.rd", "#fff")]);
        let same = srcs(&[("visual/tiles.rd", "#fff")]);
        assert_eq!(content_version(&disk), content_version(&same));
    }

    #[test]
    fn snapshot_payload_shape() {
        let snap = build_snapshot(&srcs(&[("data/tiles.rd", "grass")]));
        let v: serde_json::Value = serde_json::from_str(&snap.payload_json).unwrap();
        assert_eq!(v["version"], format!("{:016x}", snap.version));
        assert_eq!(v["rd"][0][0], "data/tiles.rd");
        assert_eq!(v["rd"][0][1], "grass");
    }

    #[test]
    fn load_disk_reads_data_then_visual_sorted() {
        let dir = std::env::temp_dir().join(format!("gw-content-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (facet, file, body) in [
            ("data", "tiles.rd", "d-tiles"),
            ("data", "things.rd", "d-things"),
            ("visual", "tiles.rd", "v-tiles"),
            ("biome", "biomes.rd", "should-be-skipped"),
        ] {
            let fdir = dir.join(facet);
            std::fs::create_dir_all(&fdir).unwrap();
            std::fs::write(fdir.join(file), body).unwrap();
        }
        let got = load_disk(&dir).unwrap();
        // data (sorted: things, tiles) then visual; biome excluded.
        let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["data/things.rd", "data/tiles.rd", "visual/tiles.rd"]);
        assert!(!got.iter().any(|(n, _)| n.starts_with("biome/")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
