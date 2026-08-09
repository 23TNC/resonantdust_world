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
    // The default player def carries NO binds (npc-host P1 moved them to the brains).
    let player_kind = b.thing_object_id("player").expect("player thing");
    assert!(b.thing_player_traits(player_kind).is_empty());
    assert!(b.player_trait_params("wolf_pack").is_some());
}

#[test]
fn brain_defs_carry_the_binds_and_pack_type_brain() {
    // npc-host P1 (F5/F10): the brain defs registry-number under TYPE_BRAIN and carry the
    // constant player-trait binds whose levels select the STAT-lane parameters.
    let b = corpus();
    for (name, need, size) in [("wolf_pack", "wolf_count", 2.0), ("bunny_fluffle", "bunny_count", 3.0)] {
        let id = b.brain_object_id(name).expect(name);
        let def = b.brain_definition_reference(name).expect("packs");
        assert_eq!(def >> 28, u32::from(resonantdust_codec::object::TYPE_BRAIN));
        let needs = b.brain_needs(id);
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0], b.gameplay_reference("need", need).unwrap());
        let binds = b.brain_player_traits(id);
        assert_eq!(binds.len(), 2, "{name}: group trait + area_of_influence");
        assert!(binds.iter().all(|t| t.constant));
        // The F10 parameter read: level-selected stat contributions from the CONSTANT binds.
        let gs = b.player_trait_stat("group_size", &binds);
        assert_eq!(gs, size, "{name}'s level-1 group_size");
    }
    // The area levels differ: wolves bind level 2 (12 tiles), bunnies level 1 (8).
    let w = b.brain_player_traits(b.brain_object_id("wolf_pack").unwrap());
    let f = b.brain_player_traits(b.brain_object_id("bunny_fluffle").unwrap());
    assert_eq!(b.player_trait_stat("area_radius", &w), 12.0);
    assert_eq!(b.player_trait_stat("area_radius", &f), 8.0);
}
