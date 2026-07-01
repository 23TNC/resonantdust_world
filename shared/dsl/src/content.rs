//! Reading the on-disk content tree into loader sources.
//!
//! The content lives under `content/`, split by facet into parallel trees:
//!   - `content/data/*.rd`   — the `:data` facets (server-side simulation).
//!   - `content/visual/*.rd` — the `:visual` facets (client-side rendering).
//! [`load`](crate::loader::load) wants `(name, source)` pairs and merges a tile's
//! facets across files, so the only thing that matters here is order: **data
//! first**, so each tile's `:data` def is indexed before its `:visual` fragment
//! folds on (and so `def_id`s are derived from the data side, the side the server
//! packs into zones).
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
  for facet in ["data", "visual"] {
    read_rd_files(&content_root.join(facet), facet, &mut sources)?;
  }
  Ok(sources)
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
