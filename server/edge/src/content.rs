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
//! **What's served.** Every `content/**/*.toml`, sorted by relative path — a TREE, so a folder
//! dropped in is a package (content-packages F1). Both sources ([`load_disk`], [`content_keys`])
//! walk it the same way, so the served corpus is identical whether it came off disk or out of the
//! bucket.
//!
//! **Server-only content is stripped by what the DATA IS, not by filename** (F2): every source goes
//! through `strip_server_only`, which removes `[[biome]]` blocks and withholds a biome-only file
//! entirely. The old rule — `name != "biomes.toml"` — was defeated by a package writing
//! `mods/foo/my-biomes.toml`, which would have shipped worldgen to every client with nothing to
//! notice. Def ids come from the registry, so load order carries no id meaning.

use crate::lock::RwRecover;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Ordered `(name, text)` source pairs, sorted by name. `name` is the path RELATIVE to the content
/// root (`"tiles.toml"`, `"mods/foo/things.toml"`), used for the version fingerprint and passed
/// through to the client for error messages.
type Sources = Vec<(String, String)>;

/// Where the gateway reads the corpus from.
pub enum ContentSource {
    /// A local content directory (dev — the bind-mounted `content/`). Walks the whole tree.
    Disk(PathBuf),
    /// The Cloudflare R2 asset bucket (deployed). Lists `<prefix>/content/` and reads every
    /// `*.toml` key it holds at any depth — the bucket is its own index.
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

/// Read the served corpus from a content dir: the whole TREE, sorted by relative path, with
/// server-only definitions stripped from every source (F2).
///
/// Delegates the walk to `resonantdust_content::content::read_content_dir` rather than keeping its
/// own — this file has hosted a private copy of the content walk before and it went stale
/// (content-toml-only I5). One reader, one set of rules.
fn load_disk(root: &Path) -> io::Result<Sources> {
    let all = resonantdust_content::content::read_content_dir(root)?;
    Ok(serve_only(all))
}

/// Apply the client-facing filter to a set of sources: strip server-only definitions, drop a source
/// left with nothing. A source that fails to parse is passed through UNCHANGED — the loader will
/// report the parse error with a proper message, and silently withholding a broken file would make
/// the client's corpus quietly incomplete instead.
fn serve_only(sources: Sources) -> Sources {
    sources
        .into_iter()
        .filter_map(|(name, text)| match resonantdust_content::content::strip_server_only(&text) {
            Ok(Some(kept)) => Some((name, kept)),
            Ok(None) => None,
            Err(_) => Some((name, text)),
        })
        .collect()
}

/// Reduce raw bucket keys to the corpus: strip the `<prefix>/content/` prefix, keep `*.toml` at
/// ANY depth, sort by relative path. **The bucket is the index** (content-toml-only F4) — there is
/// no manifest file to go stale, so this must produce the same relative names `load_disk` does or
/// the two sources would serve different corpora.
///
/// Nested keys are KEPT now (content-packages F1): a package is a folder, on disk and in the
/// bucket alike. The server-only filter is applied after fetching, on the data, not here on the
/// name — a key tells you nothing about what is inside it.
fn content_keys(keys: &[String], key_prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = keys
        .iter()
        .filter_map(|k| k.strip_prefix(key_prefix))
        .filter(|rel| rel.ends_with(".toml"))
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
    let out = serve_only(out);
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
    // The SHARED fingerprint, not a private copy. This file carried its own until
    // content-packages P1 — a duplicate that would have had to be kept in lockstep through F3's
    // basename→path change, which is exactly how the `.rd` facet walk went stale here before
    // (content-toml-only I5). One definition, one place.
    let version = resonantdust_content::content::content_version(sources);
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
    fn snapshot_payload_shape() {
        let snap = build_snapshot(&srcs(&[("tiles.toml", "grass")]));
        let v: serde_json::Value = serde_json::from_str(&snap.payload_json).unwrap();
        assert_eq!(v["version"], format!("{:016x}", snap.version));
        assert_eq!(v["toml"][0][0], "tiles.toml");
        assert_eq!(v["toml"][0][1], "grass");
    }

    #[test]
    fn content_keys_keeps_toml_at_any_depth() {
        // What a real bucket listing looks like after `toml-content`: the corpus, the
        // server-only biome file, the art manifests bin/content used to push, and the dead
        // index. Only root `*.toml` minus `biomes.toml` is the served corpus.
        let keys: Vec<String> = [
            "rd/content/tiles.toml",
            "rd/content/things.toml",
            "rd/content/materials.toml",
            "rd/content/needs.toml",
            "rd/content/biomes.toml",       // kept as a KEY — the biome filter is on data, not name
            "rd/content/manifest.json",     // not TOML
            "rd/content/visual/tiles.rd",   // a dead dialect, still in the bucket
            "rd/content/mods/foo/things.toml", // a PACKAGE — kept now (F1)
            "rd/textures/wolf/albedo.png",  // outside the prefix entirely
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let got = content_keys(&keys, "rd/content/");
        assert_eq!(
            got,
            [
                "biomes.toml",
                "materials.toml",
                "mods/foo/things.toml",
                "needs.toml",
                "things.toml",
                "tiles.toml"
            ],
            "TOML at any depth; the server-only filter runs on the DATA after fetching"
        );
    }

    #[test]
    fn load_disk_walks_the_tree_and_strips_server_only_by_data() {
        let dir = std::env::temp_dir().join(format!("gw-content-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods/foo")).unwrap();

        std::fs::write(dir.join("tiles.toml"), "[[tile]]\nname = \"grass\"\n").unwrap();
        // A biome-ONLY file at the root: withheld entirely, as `biomes.toml` always was.
        std::fs::write(dir.join("biomes.toml"), "[[biome]]\nname = \"forest\"\n").unwrap();
        std::fs::write(dir.join("manifest.json"), "not-corpus").unwrap();
        // A PACKAGE, whose file is named nothing like "biomes" yet carries biome rules alongside a
        // thing. F2: the old basename check shipped this to every client.
        std::fs::write(
            dir.join("mods/foo/stuff.toml"),
            "[[biome]]\nname = \"tundra\"\n\n[[thing]]\nname = \"snowdrift\"\n",
        )
        .unwrap();

        let got = load_disk(&dir).unwrap();
        let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["mods/foo/stuff.toml", "tiles.toml"], "the tree, minus the biome-only file");

        let package = &got.iter().find(|(n, _)| n.starts_with("mods/")).unwrap().1;
        assert!(package.contains("snowdrift"), "the package's own defs survive: {package}");
        assert!(!package.contains("tundra"), "its biome rules must NOT reach a client: {package}");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
