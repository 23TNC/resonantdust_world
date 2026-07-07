//! Reading the on-disk content tree into loader sources.
//!
//! The content lives under `content/`, split by facet into parallel trees:
//!   - `content/data/*.rd`   — the `:data` facets (server-side simulation).
//!   - `content/visual/*.rd` — the `:visual` facets (client-side rendering).
//!   - `content/biome/*.rd`  — `<biome>` generation rules (server-side worldgen).
//!   - `content/material/*.rd` — `<material>` render registry (client-side; the
//!     noise-driven hue/chroma jitter a prim's packed channels reference).
//! [`load`](crate::loader::load) wants `(name, source)` pairs and merges a tile's
//! facets across files, so the only thing that matters here is order: **data
//! first**, so each tile's `:data` def is indexed before its `:visual` fragment
//! folds on (and so `def_id`s are derived from the data side, the side the server
//! packs into zones). Biomes are their own bucket (never packed), so where they
//! land in the order doesn't affect ids — only biome-vs-biome order matters, and
//! that's within the single biome tree.
//!
//! Filesystem-backed, so it's native-only in practice — the server calls it; the
//! wasm client has no disk and feeds [`crate::loader::load`] fetched strings
//! directly.

use std::io;
use std::path::Path;

/// Read every `.rd` file under `content/data/` then `content/visual/` (each
/// sorted by name for a deterministic, content-derived id order) into the
/// `(name, source)` pairs [`crate::loader::load`] consumes. `name` is the path
/// relative to `content_root`, for error reporting.
pub fn read_content_dir(content_root: &Path) -> io::Result<Vec<(String, String)>> {
  let mut sources = Vec::new();
  for facet in ["data", "visual", "biome", "material"] {
    read_rd_files(&content_root.join(facet), facet, &mut sources)?;
  }
  Ok(sources)
}

/// A deterministic corpus fingerprint — FNV-1a over each source's **basename**
/// and **text** (NUL-separated), in the order given. No paths or timestamps, so
/// it's identical across machines and moves iff a `.rd` file's name or bytes
/// change. A consumer that reloads content compares this across reads to decide
/// whether to rebuild (the server's worldgen hot-reload; the gateway keeps an
/// equivalent over its served subset).
pub fn content_version(sources: &[(String, String)]) -> u64 {
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

/// Collect `*.rd` files in `dir` (sorted), pushing `("{label}/{file}", text)`.
/// A missing facet directory is not an error — content may carry only one side.
fn read_rd_files(dir: &Path, label: &str, out: &mut Vec<(String, String)>) -> io::Result<()> {
  if !dir.is_dir() {
    return Ok(());
  }
  let mut entries: Vec<_> = std::fs::read_dir(dir)?
    .filter_map(Result::ok)
    .map(|e| e.path())
    .filter(|p| p.extension().is_some_and(|x| x == "rd"))
    .collect();
  entries.sort();
  for path in entries {
    let text = std::fs::read_to_string(&path)?;
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    out.push((format!("{label}/{file}"), text));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::content_version;

  fn srcs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect()
  }

  #[test]
  fn version_is_deterministic_and_sensitive() {
    let a = srcs(&[("data/tiles.rd", "grass"), ("visual/tiles.rd", "#fff")]);
    assert_eq!(content_version(&a), content_version(&a));
    // a text edit moves it
    let b = srcs(&[("data/tiles.rd", "grass"), ("visual/tiles.rd", "#000")]);
    assert_ne!(content_version(&a), content_version(&b));
    // order is part of the identity (data before visual)
    let c = srcs(&[("visual/tiles.rd", "#fff"), ("data/tiles.rd", "grass")]);
    assert_ne!(content_version(&a), content_version(&c));
    // keys on basename, so a dir-prefix change alone doesn't move it
    let d = srcs(&[("x/data/tiles.rd", "grass"), ("y/visual/tiles.rd", "#fff")]);
    assert_eq!(content_version(&a), content_version(&d));
  }
}
