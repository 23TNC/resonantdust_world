//! Content serving — the gateway pulls the TOML corpus from a source (local disk
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
//! immediately (so `bin/content upload` can ping the gate).
//!
//! **What's served.** Every root `content/*.toml` except `biomes.toml`, sorted — the
//! client's `Content` bundle. Biomes are server-only worldgen and are never sent to
//! clients. Both sources ([`load_disk`], [`content_keys`]) apply that one rule, so the
//! served corpus is identical whether it came off disk or out of the bucket. Def ids are
//! explicit in the TOML, so load order carries no id meaning.

use crate::lock::RwRecover;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Ordered `(name, text)` source pairs, sorted by name. `name` is the corpus basename
/// (e.g. `"tiles.toml"`), used for the version fingerprint and passed through to the
/// client for error messages.
type Sources = Vec<(String, String)>;

/// Where the gateway reads the corpus from.
pub enum ContentSource {
    /// A local content directory (dev — the bind-mounted `content/`). Reads
    /// top-level `*.toml`, sorted.
    Disk(PathBuf),
    /// The Cloudflare R2 asset bucket (deployed). Lists `<prefix>/content/` and
    /// reads every root `*.toml` key it holds — the bucket is its own index.
    R2(R2Config),
}

/// R2 access parameters (a subset of `bin/content`'s config, from the environment).
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

/// Read the served corpus from a content dir: top-level `*.toml`, sorted, minus
/// `biomes.toml` (server-only worldgen; ids are explicit, so serving order carries no id
/// meaning).
///
/// Root-only and TOML-only, matching [`content_keys`] on the R2 side. The `.rd` facet walk
/// (`data` → `visual` → `material`) that used to sit under this as a fallback was a fourth
/// private copy of the content walk and died with the dialect (content-toml-only I5).
fn load_disk(root: &Path) -> io::Result<Sources> {
    let mut toml: Vec<PathBuf> = std::fs::read_dir(root)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .filter(|p| p.file_name().is_some_and(|n| n != "biomes.toml"))
        .collect();
    toml.sort();
    let mut out = Sources::new();
    for path in toml {
        let text = std::fs::read_to_string(&path)?;
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        out.push((file.to_string(), text));
    }
    Ok(out)
}

/// Reduce raw bucket keys to the served corpus: strip the `<prefix>/content/` prefix,
/// keep root-level `*.toml` only, drop `biomes.toml`, sort. **The bucket is the index**
/// (content-toml-only F4) — there is no manifest file to go stale, so this must apply
/// exactly `load_disk`'s rules or the two sources would serve different corpora.
///
/// Keys arriving from a subdirectory are dropped rather than flattened: the disk walk is
/// root-only, and a nested key would collide with a root basename in the fingerprint
/// (which hashes basenames, not paths).
fn content_keys(keys: &[String], key_prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = keys
        .iter()
        .filter_map(|k| k.strip_prefix(key_prefix))
        .filter(|rel| !rel.contains('/') && rel.ends_with(".toml") && *rel != "biomes.toml")
        .map(str::to_string)
        .collect();
    names.sort();
    names
}

/// List `<prefix>/content/` and fetch every corpus key it holds. R2 is S3-compatible, so
/// `ListObjectsV2` over the prefix makes the bucket self-describing — the old
/// `manifest.json` index listed five `.rd` keys deleted by `toml-content` P6 and broke
/// this path silently (I2). Paginated, because correctness costs one loop.
async fn load_r2(cfg: &R2Config) -> Result<Sources, String> {
    let key_prefix = format!("{}/content/", cfg.prefix);
    let keys = r2_list_keys(cfg, &key_prefix).await?;
    let mut out = Sources::new();
    for name in content_keys(&keys, &key_prefix) {
        let text = r2_get_text(cfg, &name).await?;
        out.push((name, text));
    }
    if out.is_empty() {
        return Err(format!("R2 LIST {key_prefix}: no *.toml corpus keys"));
    }
    Ok(out)
}

/// Every object key under `key_prefix`, following continuation tokens. Keys come back
/// percent-encoded (`rusty-s3` sets `encoding-type=url`), so each is decoded before it
/// reaches the caller.
async fn r2_list_keys(cfg: &R2Config, key_prefix: &str) -> Result<Vec<String>, String> {
    use rusty_s3::S3Action;
    use rusty_s3::actions::ListObjectsV2;

    let (bucket, creds) = r2_bucket(cfg)?;
    let mut keys = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut action = bucket.list_objects_v2(Some(&creds));
        action.with_prefix(key_prefix.to_string());
        if let Some(t) = &token {
            action.with_continuation_token(t.clone());
        }
        let signed = action.sign(Duration::from_secs(60));
        let body = reqwest::get(signed)
            .await
            .map_err(|e| format!("R2 LIST {key_prefix}: {e}"))?
            .error_for_status()
            .map_err(|e| format!("R2 LIST {key_prefix}: {e}"))?
            .text()
            .await
            .map_err(|e| format!("R2 LIST {key_prefix} body: {e}"))?;
        let parsed = ListObjectsV2::parse_response(&body)
            .map_err(|e| format!("R2 LIST {key_prefix} parse: {e}"))?;
        for c in &parsed.contents {
            keys.push(
                percent_encoding::percent_decode_str(&c.key).decode_utf8_lossy().into_owned(),
            );
        }
        match parsed.next_continuation_token {
            Some(t) => token = Some(t),
            None => break,
        }
    }
    Ok(keys)
}

/// The signing pair both R2 actions need.
fn r2_bucket(cfg: &R2Config) -> Result<(rusty_s3::Bucket, rusty_s3::Credentials), String> {
    use rusty_s3::{Bucket, Credentials, UrlStyle};

    let base = url::Url::parse(&cfg.endpoint).map_err(|e| format!("bad R2 endpoint: {e}"))?;
    let bucket = Bucket::new(base, UrlStyle::Path, cfg.bucket.clone(), cfg.region.clone())
        .map_err(|e| format!("R2 bucket: {e}"))?;
    let creds = Credentials::new(cfg.access_key.clone(), cfg.secret_key.clone());
    Ok((bucket, creds))
}

/// GET one object's text from R2 at `<prefix>/content/<rel>` via a short-lived
/// SigV4-presigned URL (R2 is S3-compatible). `rel` is a corpus basename like
/// `"tiles.toml"`.
async fn r2_get_text(cfg: &R2Config, rel: &str) -> Result<String, String> {
    use rusty_s3::S3Action;

    let (bucket, creds) = r2_bucket(cfg)?;
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
/// identical across machines and moves iff a served corpus file's name or bytes
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

/// Build the `/content` payload from sources: `{ "version": "<hex>", "toml": [[name,
/// text], …] }`. The client feeds `toml` straight into `new Content(names, sources)`.
///
/// The key was `rd` until content-toml-only P4 — named after a dialect deleted two streams
/// ago. Renamed with all three consumers in one commit (F6); there is one server and one
/// client, deployed together, so no dual-read window was needed.
fn build_snapshot(sources: &Sources) -> Snapshot {
    let version = content_version(sources);
    let payload = serde_json::json!({
        "version": format!("{version:016x}"),
        "toml": sources,
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
        self.current.read_r().payload_json.clone()
    }

    /// The `/content-version` fingerprint as 16 hex chars.
    pub fn version_hex(&self) -> String {
        format!("{:016x}", self.current.read_r().version)
    }

    /// Re-read the source and, if the fingerprint moved, swap the snapshot in.
    /// Returns whether it changed. The new snapshot is built before the swap, so a
    /// transient read error leaves the live corpus untouched.
    pub async fn refresh(&self) -> Result<bool, String> {
        let sources = self.source.load().await?;
        let snap = build_snapshot(&sources);
        if snap.version == self.current.read_r().version {
            return Ok(false);
        }
        *self.current.write_r() = Arc::new(snap);
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
        let a = srcs(&[("things.toml", "grass"), ("tiles.toml", "#fff")]);
        assert_eq!(content_version(&a), content_version(&a), "same corpus → same hash");

        // a text change moves the fingerprint
        let b = srcs(&[("things.toml", "grass"), ("tiles.toml", "#000")]);
        assert_ne!(content_version(&a), content_version(&b));

        // order matters — it is part of the corpus identity
        let c = srcs(&[("tiles.toml", "#fff"), ("things.toml", "grass")]);
        assert_ne!(content_version(&a), content_version(&c));
    }

    #[test]
    fn version_uses_basename_not_path() {
        // Disk serves bare basenames and R2 strips its key prefix to the same, so the
        // two sources agree by construction; the basename keying makes that explicit.
        let disk = srcs(&[("tiles.toml", "#fff")]);
        let keyed = srcs(&[("some/prefix/tiles.toml", "#fff")]);
        assert_eq!(content_version(&disk), content_version(&keyed));
    }

    #[test]
    fn snapshot_payload_shape() {
        let snap = build_snapshot(&srcs(&[("tiles.toml", "grass")]));
        let v: serde_json::Value = serde_json::from_str(&snap.payload_json).unwrap();
        assert_eq!(v["version"], format!("{:016x}", snap.version));
        assert_eq!(v["toml"][0][0], "tiles.toml");
        assert_eq!(v["toml"][0][1], "grass");
    }

    #[test]
    fn content_keys_keeps_root_toml_only() {
        // What a real bucket listing looks like after `toml-content`: the corpus, the
        // server-only biome file, the art manifests bin/content used to push, and the dead
        // index. Only root `*.toml` minus `biomes.toml` is the served corpus.
        let keys: Vec<String> = [
            "rd/content/tiles.toml",
            "rd/content/things.toml",
            "rd/content/materials.toml",
            "rd/content/needs.toml",
            "rd/content/biomes.toml",     // server-only worldgen — never served
            "rd/content/manifest.json",   // the index this stream deleted
            "rd/content/visual/tiles.rd", // a dead dialect, still in the bucket
            "rd/content/visual/x.toml",   // nested: the disk walk is root-only
            "rd/textures/wolf/albedo.png", // outside the prefix entirely
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let got = content_keys(&keys, "rd/content/");
        assert_eq!(got, ["materials.toml", "needs.toml", "things.toml", "tiles.toml"]);
    }

    #[test]
    fn load_disk_reads_root_toml_sorted() {
        let dir = std::env::temp_dir().join(format!("gw-content-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, body) in [
            ("tiles.toml", "t"),
            ("things.toml", "th"),
            ("biomes.toml", "should-be-skipped"), // server-only worldgen
            ("manifest.json", "not-corpus"),
        ] {
            std::fs::write(dir.join(file), body).unwrap();
        }
        // A nested TOML is invisible: the walk is root-only, matching `content_keys`.
        std::fs::create_dir_all(dir.join("visual")).unwrap();
        std::fs::write(dir.join("visual/nested.toml"), "nope").unwrap();

        let got = load_disk(&dir).unwrap();
        let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["things.toml", "tiles.toml"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
