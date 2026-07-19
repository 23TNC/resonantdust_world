//! `outline` — the art-pipeline CLI around [`resonantdust_geometry::generate`]. Reads a sprite PNG
//! (alpha = silhouette), emits its [`Sidecar`](resonantdust_geometry::Sidecar) (boundary polygons +
//! earcut triangulation) as one line of JSON to stdout. `bin/art` shells out to it per variant leaf;
//! `bin/lib/outline.py` merges the JSON into that leaf's `meta.json`.
//!
//! Usage: `outline <sprite.png>`  → JSON on stdout, or a message on stderr + non-zero exit.

use std::io::Read;

use resonantdust_geometry::{generate, Options};

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: outline <sprite.png>");
            std::process::exit(2);
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = std::fs::File::open(&path).and_then(|mut f| f.read_to_end(&mut bytes)) {
        eprintln!("outline: read {path}: {e}");
        std::process::exit(1);
    }
    match generate(&bytes, &Options::default()) {
        Ok(sidecar) => println!("{}", serde_json::to_string(&sidecar).expect("Sidecar serializes")),
        Err(e) => {
            eprintln!("outline: {path}: {e}");
            std::process::exit(1);
        }
    }
}
