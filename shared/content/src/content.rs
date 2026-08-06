//! Reading the on-disk content tree into loader sources.
//!
//! The corpus is **`content/**/*.toml`** — a TREE, walked recursively, so a folder dropped in is a
//! package (work `2026-08-05-content-packages` F1). Sources are named by their path RELATIVE to the
//! content root (`mods/foo/things.toml`), sorted, so the order is deterministic across machines.
//!
//! Order carries no id meaning: ids come from the definition registry, not from corpus position.
//! What order still decides is the biome array's evaluation priority, and biomes are ordered within
//! their own file, which sorting preserves.
//!
//! Filesystem-backed, so it's native-only in practice — the server calls it; the
//! wasm client has no disk and feeds [`crate::loader::load`] fetched strings
//! directly.

use std::io;
use std::path::Path;

/// Read the corpus — every `*.toml` under `content_root`, at any depth, sorted by relative path —
/// into the `(name, source)` pairs [`crate::loader::load`] consumes.
///
/// A missing root is not an error (an empty corpus loads), matching the previous behaviour.
pub fn read_content_dir(content_root: &Path) -> io::Result<Vec<(String, String)>> {
  let mut sources = Vec::new();
  read_tree(content_root, content_root, &mut sources)?;
  // Sort by NAME rather than by traversal order: `read_dir` order is filesystem-defined, and two
  // machines walking the same tree must produce the same corpus in the same order.
  sources.sort_by(|a, b| a.0.cmp(&b.0));
  Ok(sources)
}

/// A deterministic corpus fingerprint — FNV-1a over each source's **relative path** and **text**
/// (NUL-separated), in the order given. No absolute paths or timestamps, so it is identical across
/// machines and moves iff a corpus file's path or bytes change. A consumer that reloads content
/// compares this across reads to decide whether to rebuild (the server's worldgen hot-reload).
///
/// **The path, not the basename** (F3). Hashing basenames was correct while the corpus was flat and
/// is a COLLISION once it nests: `mods/a/things.toml` and `mods/b/things.toml` would feed the hash
/// identically, and moving a file between packages would not change the fingerprint at all — so a
/// hot-reload poll would never notice. Disk and R2 both produce the same relative path (one strips
/// the content root, the other the key prefix), which is the property the basename hack existed to
/// preserve.
pub fn content_version(sources: &[(String, String)]) -> u64 {
  let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
  let mut feed = |bytes: &[u8]| {
    for &b in bytes {
      h ^= b as u64;
      h = h.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
    }
  };
  for (name, text) in sources {
    feed(name.as_bytes());
    feed(b"\0");
    feed(text.as_bytes());
    feed(b"\0");
  }
  h
}

/// Recurse `dir`, pushing `(relative_path, text)` for every `*.toml` found at any depth. A missing
/// directory is not an error. Paths are normalized to `/` separators so a source's name is the same
/// string on every platform — it is hashed into the corpus fingerprint and shipped to clients.
fn read_tree(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> io::Result<()> {
  if !dir.is_dir() {
    return Ok(());
  }
  for entry in std::fs::read_dir(dir)?.filter_map(Result::ok) {
    let path = entry.path();
    if path.is_dir() {
      read_tree(root, &path, out)?;
    } else if path.extension().is_some_and(|x| x == "toml") {
      let rel = path.strip_prefix(root).unwrap_or(&path);
      let name = rel.components().filter_map(|c| c.as_os_str().to_str()).collect::<Vec<_>>().join("/");
      out.push((name, std::fs::read_to_string(&path)?));
    }
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
    let a = srcs(&[("things.toml", "grass"), ("tiles.toml", "#fff")]);
    assert_eq!(content_version(&a), content_version(&a));
    // a text edit moves it
    let b = srcs(&[("things.toml", "grass"), ("tiles.toml", "#000")]);
    assert_ne!(content_version(&a), content_version(&b));
    // order is part of the identity
    let c = srcs(&[("tiles.toml", "#fff"), ("things.toml", "grass")]);
    assert_ne!(content_version(&a), content_version(&c));
  }

  #[test]
  fn the_fingerprint_keys_on_the_path_so_packages_cannot_collide() {
    // F3: two packages may each carry a `things.toml`. Keyed on the basename they would hash
    // identically — and moving a file between packages would change NOTHING, so the hot-reload
    // poll would never fire. The path is what distinguishes them.
    let a = srcs(&[("mods/a/things.toml", "wolf")]);
    let b = srcs(&[("mods/b/things.toml", "wolf")]);
    assert_ne!(content_version(&a), content_version(&b), "same file name, different package");

    // Moving a file is a change.
    let root = srcs(&[("things.toml", "wolf")]);
    assert_ne!(content_version(&root), content_version(&a), "root vs package is a change");
  }

  #[test]
  fn a_tree_reads_relative_paths_sorted() {
    let dir = std::env::temp_dir().join(format!("rd-content-tree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("mods/foo")).unwrap();
    std::fs::write(dir.join("a.toml"), "root").unwrap();
    std::fs::write(dir.join("mods/foo/b.toml"), "package").unwrap();
    std::fs::write(dir.join("mods/foo/notes.md"), "ignored").unwrap();

    let got = super::read_content_dir(&dir).unwrap();
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["a.toml", "mods/foo/b.toml"], "relative paths, sorted, non-TOML skipped");
    std::fs::remove_dir_all(&dir).unwrap();
  }
}
