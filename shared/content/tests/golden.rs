//! The GOLDEN ORACLE (toml-content P0/F5): a canonical dump of everything the Bundle
//! serves — every registry, every flat table, a worldgen sweep, the needs probes —
//! committed as `tests/golden/corpus.txt` and compared on every test run.
//!
//! It began as a MIGRATION gate (the TOML loader had to reproduce the `.rd` fixture
//! byte-identically before any consumer swapped). The `.rd` side died with the DSL in
//! toml-content P6; the fixture outlived it as the standing guard that a corpus or loader
//! edit changes exactly what the author intended. The `.rd` half of this file went with it
//! (conditions I2 — it had taken the `BLESS_GOLDEN` branch down with it).
//!
//! Regenerate deliberately (never as a side effect): `BLESS_GOLDEN=1 cargo test -p
//! resonantdust-content --test golden`.

use resonantdust_content::loader::{load, Bundle};
use resonantdust_content::needs_eval;
use std::fmt::Write as _;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/corpus.txt")
}

/// The repo's authored TOML corpus, or `None` in a packaged build without it.
fn corpus() -> Option<Bundle> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    if !root.exists() {
        return None; // packaged build without the repo corpus — the oracle only runs in-repo
    }
    // The SHARED reader (content-packages I5). This test had its own flat `read_dir`, so the
    // moment the corpus became a tree the oracle stopped seeing packages — it would have passed
    // happily while a mod added definitions it never checked. An oracle that reads the corpus
    // differently from the code is not an oracle.
    let sources = resonantdust_content::content::read_content_dir(&root).expect("read content/");
    if sources.is_empty() {
        return None;
    }
    Some(load(&sources).expect("the TOML corpus loads clean"))
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
    sec("conditions", b.condition_names().join("\n"), &mut out);
    sec(
        "biomes (name subtype, evaluation order)",
        b.biome_names()
            .iter()
            .map(|n| format!("{n} {}", b.biome_subtype_id(n).unwrap_or(0)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );

    // ── name → id RETIRED (definition-registry F15) ───────────────────────────────────────
    //
    // This section used to dump `name → kind_id` for tiles and things. It cannot mean anything
    // now: the corpus carries no ids, so a Bundle loaded here (with no registry injected) numbers
    // them by POSITION — the section would assert that corpus order has not changed while reading
    // as though it proved identity. This stream has already caught two tests passing for that
    // reason; a third would be worse than none.
    //
    // What guards identity instead is the REGISTRY, and it is a constraint rather than a fixture:
    // `index.definitions` has `id` as its primary key and refuses a second id for one
    // (tuple, version), so an EXISTING numbering can never be contradicted. A fresh DB seeded from
    // a reordered corpus does get different numbers — correctly, because a fresh DB is a fresh
    // world.
    //
    // MATERIALS still author ids and are still pinned by the registry dumped above, where the id
    // IS the line number. Needs/conditions (and the interactions categories) stopped authoring
    // ids in `2026-08-06-interactions` F1 — their identity is the registry-numbered
    // `gameplay/<category>/<name>/default` tuple, and the SEED refs are dumped below.

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
    // (the "thing speeds" section left with the `speed` field — input-rework F8: pace is
    // the DERIVED ground_speed, whose inputs are already dumped as trait bindings.)

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
    sec("condition_params", format!("{:#?}", b.condition_params_all()), &mut out);

    // ── the gameplay categories (interactions P1) — registries, params, seed refs, carriers ──
    sec("traits", b.trait_names().join("\n"), &mut out);
    sec("interactions", b.interaction_names().join("\n"), &mut out);
    sec("affordances", b.affordance_names().join("\n"), &mut out);
    sec(
        "trait_params",
        b.trait_names()
            .iter()
            .map(|n| format!("{n}: {:?}", b.trait_params(n)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    sec(
        "interaction_params",
        b.interaction_names()
            .iter()
            .map(|n| format!("{n}: {:?}", b.interaction_params(n)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    sec(
        "affordance_params",
        b.affordance_names()
            .iter()
            .map(|n| format!("{n}: {:?}", b.affordance_params(n)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    sec(
        "stat_params",
        b.stat_names()
            .iter()
            .map(|n| format!("{n}: {:?}", b.stat_params(n)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    // The SEED gameplay refs — what a registry-less boot resolves, and exactly what a fresh
    // registry allocates from (the migration-proof posture the kind seeds established).
    let mut refs = String::new();
    for (category, names) in [
        ("need", b.need_names()),
        ("condition", b.condition_names()),
        ("trait", b.trait_names()),
        ("interaction", b.interaction_names()),
        ("affordance", b.affordance_names()),
        ("stat", b.stat_names()),
    ] {
        for n in names {
            let r = b.gameplay_reference(category, n);
            let _ = writeln!(refs, "gameplay/{category}/{n} {:?}", r.map(|r| format!("{r:#010x}")));
        }
    }
    sec("gameplay seed refs", refs, &mut out);
    sec(
        "tile interactions (name → bindings)",
        b.tile_names()
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{n} {:?}", b.tile_interactions(i as u16 + 1)))
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );
    sec(
        "thing traits + interactions",
        b.thing_names()
            .iter()
            .enumerate()
            .map(|(i, n)| {
                format!(
                    "{n} traits={:?} interactions={:?}",
                    b.thing_traits(i as u16 + 1),
                    b.thing_interactions(i as u16 + 1)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        &mut out,
    );

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
    // Rows are the PACKED stat-model shapes: needs `(value:16|key:16, set_tic)` with the
    // value u16 fixed-point on thirst's authored 0..100 domain (F4), stored conditions
    // `(remaining_at_write:16|key:16, written_tic)` (F3). Refs resolve through the seed.
    let thirst = b.gameplay_reference("need", "thirst").expect("thirst ref");
    let quenched = b.gameplay_reference("condition", "quenched").expect("quenched ref");
    let tp = b.need_params("thirst").expect("thirst params");
    let qp = b.condition_params("quenched").expect("quenched params");
    let nrow = |v: f32| {
        resonantdust_codec::object::pack_gameplay_row(
            thirst,
            resonantdust_codec::value::quantize(v, tp.min as f32, tp.max as f32),
        )
    };
    let qrow = resonantdust_codec::object::pack_gameplay_row(quenched, qp.duration as u16);
    let probes: [(&str, Vec<(u32, u16)>, Vec<(u32, u16)>, u16); 4] = [
        ("full", vec![(nrow(100.0), 0)], vec![], 0),
        ("mid", vec![(nrow(25.0), 0)], vec![], 0),
        ("empty", vec![(nrow(0.0), 0)], vec![], 0),
        ("timed", vec![], vec![(qrow, 0)], 100),
    ];
    let mut needs = String::new();
    for (name, rows, grants, now) in &probes {
        let active = needs_eval::active_conditions(b, &[], rows, grants, *now);
        let _ = writeln!(
            needs,
            "{name}: active={:?} mood={:?} next={:?}",
            active.iter().map(|m| (m.condition_id, m.mood, m.remaining)).collect::<Vec<_>>(),
            needs_eval::mood(&active),
            needs_eval::next_crossing_tic(b, &[], rows, grants, *now),
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

#[test]
fn the_dump_is_deterministic() {
    let Some(b) = corpus() else { return };
    assert_eq!(dump(&b), dump(&b), "two dumps of one bundle must be byte-identical");
}

// (stat-model I10's derived-vs-authored speed guard died WITH the `speed` field — the
// derived-value pin lives in the npc crate's `the_wolfs_derived_ground_speed_is_the_old_
// authored_pace`, which asserts walks level 2 still derives 12.0.)
