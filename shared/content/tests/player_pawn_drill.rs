//! player-pawns P4 — the eval drill's pinned half: a zeroed `wolf_count` BANDS (packless
//! activates through the ONE eval), and the player lane loads (the `player` def carries its
//! constant `wolf_pack` bind in the DEDICATED lane).

use resonantdust_content::loader::{load, Bundle};
use resonantdust_content::needs_eval;

fn corpus() -> Bundle {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let sources = resonantdust_content::content::read_content_dir(&root).expect("read content/");
    load(&sources).expect("the repo corpus loads clean")
}

#[test]
fn a_zeroed_wolf_count_bands_packless_and_never_kills() {
    let b = corpus();
    let nref = b.gameplay_reference("need", "wolf_count").expect("wolf_count");
    // The live drill's row: value 0 at set_tic 35303 (deplete 0 — the value IS current).
    let row = resonantdust_codec::object::pack_gameplay_row(nref, 0);
    let need_rows = [(row, 35303u16)];
    let sat = needs_eval::need_satisfaction(&b, "wolf_count", &[], &need_rows, &[], 35303)
        .expect("evaluates");
    assert_eq!(sat, 0.0, "a zeroed count evaluates to the floor");
    // The band: packless is authored lo=0 hi=1 — ACTIVE at 0, via the ONE eval.
    let conds = needs_eval::active_conditions(&b, &[], &need_rows, &[], 35303);
    let packless = b.gameplay_reference("condition", "packless").expect("packless");
    assert!(conds.iter().any(|c| c.condition_id == packless), "packless activates: {conds:?}");
    // The dedicated player-trait lane carries the player def's constant bind.
    let player_kind = b.thing_object_id("player").expect("player thing");
    let binds = b.thing_player_traits(player_kind);
    assert_eq!(binds.len(), 1);
    assert!(binds[0].constant && binds[0].name == "wolf_pack");
    assert!(b.player_trait_params("wolf_pack").is_some());
}
