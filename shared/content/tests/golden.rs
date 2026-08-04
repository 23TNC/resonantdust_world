//! The GOLDEN ORACLE (toml-content P0/F5): a canonical dump of everything the Bundle
//! serves — every registry, every flat table, a worldgen sweep, the needs probes —
//! committed as `tests/golden/corpus.txt` and compared on every test run.
//!
//! The TOML loader must reproduce this file BYTE-IDENTICALLY from the translated corpus
//! before any consumer swaps (F5); until then this test also pins the `.rd` corpus so the
//! migration target cannot drift while the work is in flight. Migration-scoped: it dies
//! with the old loader in P6.
//!
//! Regenerate deliberately (never as a side effect): `BLESS_GOLDEN=1 cargo test -p
//! resonantdust-content --test golden`.

use resonantdust_content::loader::{load, Bundle};
use resonantdust_content::needs_eval;
use std::fmt::Write as _;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/corpus.txt")
}

fn corpus() -> Option<Bundle> {
    // The `.rd` corpus EXPLICITLY (read_content_dir now prefers TOML — P5): the oracle's
    // whole point is comparing the two dialects, so each side names its own files.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    if !root.exists() {
        return None; // packaged build without the repo corpus — the oracle only runs in-repo
    }
    let mut sources = Vec::new();
    for facet in ["data", "visual", "biome", "material"] {
        let dir = root.join(facet);
        if !dir.is_dir() {
            continue;
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .expect("read facet dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "rd"))
            .collect();
        files.sort();
        for p in files {
            sources.push((
                format!("{facet}/{}", p.file_name().unwrap().to_string_lossy()),
                std::fs::read_to_string(&p).expect("read .rd"),
            ));
        }
    }
    if sources.is_empty() {
        return None; // post-deletion checkout — the oracle retired with the DSL
    }
    Some(load(&sources).expect("the repo corpus loads clean"))
}

/// Serialize floats via `{:?}` (shortest round-trip form — deterministic and exact).
fn floats(v: &[f64]) -> String {
    let mut s = String::new();
    for (i, f) in v.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{f:?}");
    }
    s
}

fn dump(b: &Bundle) -> String {
    let mut out = String::new();
    let mut sec = |name: &str, body: String, out: &mut String| {
        let _ = writeln!(out, "== {name} ==");
        out.push_str(&body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    };

    // ── registries (name per line, in id order — the id IS the line number) ──
    sec("tiles", b.tile_names().join("\n"), &mut out);
    sec("things", b.thing_names().join("\n"), &mut out);
    sec("materials", b.material_names().join("\n"), &mut out);
    sec("needs", b.need_names().join("\n"), &mut out);
    sec("moodlets", b.moodlet_names().join("\n"), &mut out);
    sec(
        "biomes (name subtype, evaluation order)",
        b.biome_names()
            .iter()
            .map(|n| format!("{n} {}", b.biome_subtype_id(n).unwrap_or(0)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );

    // ── per-def scalar/colour lookups ──
    sec(
        "tile colors",
        b.tile_names()
            .iter()
            .map(|n| format!("{n} {:?}", b.tile_color_bg(n)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    sec(
        "thing colors",
        b.thing_names()
            .iter()
            .map(|n| format!("{n} {:?}", b.thing_color_bg(n)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    sec(
        "thing speeds (0 = unauthored)",
        floats(&b.thing_speeds()),
        &mut out,
    );

    // ── flat tables, exactly as consumers fetch them ──
    sec("tile_texture_stems", b.tile_texture_stems().join("\n"), &mut out);
    sec("tile_builds", b.tile_builds().join("\n"), &mut out);
    sec("tile_heights", floats(&b.tile_heights()), &mut out);
    sec("tile_lighting_lanes", floats(&b.tile_lighting_lanes()), &mut out);
    sec("thing_texture_stems", b.thing_texture_stems().join("\n"), &mut out);
    sec("thing_layout", floats(&b.thing_layout()), &mut out);
    sec("thing_light", floats(&b.thing_light()), &mut out);
    sec("thing_subframe", floats(&b.thing_subframe()), &mut out);
    sec("thing_needs_table", floats(&b.thing_needs_table()), &mut out);
    sec(
        "tile_packed_channels",
        format!("{:?}", b.tile_packed_channels()),
        &mut out,
    );
    sec(
        "thing_packed_channels",
        format!("{:?}", b.thing_packed_channels()),
        &mut out,
    );
    sec("material_params", format!("{:#?}", b.material_params_all()), &mut out);
    sec("need_params", format!("{:#?}", b.need_params_all()), &mut out);
    sec("moodlet_params", format!("{:#?}", b.moodlet_params_all()), &mut out);

    // ── the visual PARTS skeletons (per thing — the human's body+head, the wolf's one) ──
    sec(
        "thing visual parts",
        b.thing_names()
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let v = b.visual_for_object(i as u16 + 1);
                format!(
                    "{n}: {}",
                    v.map(|v| format!(
                        "{} part(s) {:?}",
                        v.parts.len(),
                        v.parts
                            .iter()
                            .map(|p| (p.part, p.scale, p.texture.clone()))
                            .collect::<Vec<_>>()
                    ))
                    .unwrap_or_else(|| "-".into())
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );

    // ── the worldgen sweep: every 0.05 step of (temperature, humidity, elevation),
    //    seed = the cell's index — covers every band boundary + the scatter rand paths.
    let mut grid = String::new();
    let mut idx: u64 = 0;
    for t in 0..=20 {
        for h in 0..=20 {
            for e in 0..=20 {
                let dims = [f64::from(t) * 0.05, f64::from(h) * 0.05, f64::from(e) * 0.05];
                let g = b.generate(&dims, idx);
                let _ = writeln!(
                    grid,
                    "{t},{h},{e} {} {} {}",
                    g.biome.as_deref().unwrap_or("-"),
                    g.tile.as_deref().unwrap_or("-"),
                    g.thing1.as_deref().unwrap_or("-"),
                );
                idx += 1;
            }
        }
    }
    sec("worldgen sweep (t,h,e biome tile thing1)", grid, &mut out);

    // ── the needs probes (the wasm-probe fixtures, against the REAL corpus) ──
    let probes: [(&str, Vec<(u8, u8, u16)>, Vec<(u8, u16)>, u16); 4] = [
        ("full", vec![(1, 255, 0)], vec![], 0),
        ("mid", vec![(1, 64, 0)], vec![], 0),
        ("empty", vec![(1, 0, 0)], vec![], 0),
        ("timed", vec![], vec![(3, 0)], 100),
    ];
    let mut needs = String::new();
    for (name, rows, grants, now) in &probes {
        let active = needs_eval::active_moodlets(b, rows, grants, *now);
        let _ = writeln!(
            needs,
            "{name}: active={:?} mood={:?} next={:?}",
            active.iter().map(|m| (m.moodlet_id, m.mood, m.remaining)).collect::<Vec<_>>(),
            needs_eval::mood(&active),
            needs_eval::next_crossing_tic(b, rows, grants, *now),
        );
    }
    sec("needs probes", needs, &mut out);

    out
}

#[test]
fn the_golden_fixture_matches_the_corpus() {
    let Some(b) = corpus() else { return };
    let now = dump(&b);
    let path = fixture_path();
    if std::env::var("BLESS_GOLDEN").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &now).unwrap();
        eprintln!("golden fixture blessed: {} bytes", now.len());
        return;
    }
    let want = std::fs::read_to_string(&path)
        .expect("tests/golden/corpus.txt missing — run with BLESS_GOLDEN=1 to create it");
    if want != now {
        // Print the FIRST divergent line so the failure names its section.
        for (i, (w, n)) in want.lines().zip(now.lines()).enumerate() {
            if w != n {
                panic!("golden mismatch at line {}:\n  fixture: {w}\n  corpus:  {n}", i + 1);
            }
        }
        panic!(
            "golden mismatch: lengths differ (fixture {} lines, corpus {} lines)",
            want.lines().count(),
            now.lines().count()
        );
    }
}

/// THE GATE (P4/F5): the TOML corpus must reproduce the `.rd` fixture byte-identically.
/// Nothing swaps until this is green; the `.rd` corpus + loader die one commit after.
#[test]
fn the_toml_corpus_matches_the_same_fixture() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    if !root.exists() {
        return;
    }
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&root)
        .expect("read content/")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().to_string(),
                std::fs::read_to_string(e.path()).expect("read .toml"),
            )
        })
        .collect();
    if sources.is_empty() {
        return; // pre-conversion checkout
    }
    sources.sort();
    let b = load(&sources).expect("the TOML corpus loads clean");
    let now = dump(&b);
    let want = std::fs::read_to_string(fixture_path()).expect("fixture exists");
    if want != now {
        for (i, (w, n)) in want.lines().zip(now.lines()).enumerate() {
            if w != n {
                panic!(
                    "TOML corpus diverges from the .rd fixture at line {}:\n  .rd:  {w}\n  toml: {n}",
                    i + 1
                );
            }
        }
        panic!(
            "TOML corpus diverges: lengths differ (fixture {} lines, toml {} lines)",
            want.lines().count(),
            now.lines().count()
        );
    }
}

#[test]
fn the_dump_is_deterministic() {
    let Some(b) = corpus() else { return };
    assert_eq!(dump(&b), dump(&b), "two dumps of one bundle must be byte-identical");
}
