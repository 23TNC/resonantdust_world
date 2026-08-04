//! Load the real on-disk content tree and dump each tile's id + colour — a quick
//! end-to-end check of the loader against `content/{data,visual}/*.rd`.
//!
//! Run with the repo root reachable so it can find `content/`:
//!   cargo run -p resonantdust-content --example dump_tiles -- <content_root>

use std::path::PathBuf;

fn main() {
  let root: PathBuf = std::env::args().nth(1).unwrap_or_else(|| "content".into()).into();
  let sources = resonantdust_content::content::read_content_dir(&root)
    .unwrap_or_else(|e| panic!("read {}: {e}", root.display()));
  println!("read {} source files from {}", sources.len(), root.display());
  let bundle = resonantdust_content::load(&sources).expect("load content");
  for name in bundle.tile_names() {
    let id = bundle.tile_def_id(name).unwrap();
    let color = bundle.tile_color_bg(name);
    println!("  def_id {id:>3}  {name:<10} bg = {:?}", color.map(|c| format!("#{c:06x}")));
  }
}
