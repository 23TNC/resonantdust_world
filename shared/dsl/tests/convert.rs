//! The `.rd` → `.toml` corpus CONVERTER (toml-content P3) — throwaway, deleted with the
//! DSL in P6. Run deliberately: `CONVERT_TOML=1 cargo test -p resonantdust-dsl --test
//! convert` (docker mounts the repo; output lands in `content/*.toml`).
//!
//! Ids are pinned to today's positional values (F1). Biomes are NOT emitted here — their
//! `.rd` bodies are code; the 7 biomes were translated BY HAND into `content/biomes.toml`
//! and the golden sweep (9,261 cells) proves the translation exact. Comments worth
//! carrying were hand-moved (P3 item 2), so this tool emits data only.

use resonantdust_dsl::loader::{
  load, Bundle, DirFrame, VisualPart, ROTATIONS_PER_DEF, VARIANTS_PER_DEF,
};
use std::fmt::Write as _;

fn corpus() -> Option<Bundle> {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
  if !root.exists() {
    return None;
  }
  let sources = resonantdust_dsl::content::read_content_dir(&root).expect("read content/");
  Some(load(&sources).expect("the repo corpus loads clean"))
}

fn hex(c: u32) -> String {
  format!("\"#{c:06x}\"")
}

fn f(v: f64) -> String {
  format!("{v:?}")
}

/// Reconstruct the MINIMAL authored subframe keys from the expanded `[16][16]` array —
/// a residual encoding against the fallback chain: base ← the never-authored `[15][15]`
/// slot, per-rotation rows ← `[15][r]`, per-variant rows ← `[v][15]`, and any slot the
/// chain then mispredicts gets a fully-specific `v.r` key. Exact by construction (the
/// golden gate re-checks the expansion).
fn subframe_keys(frames: &[[DirFrame; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF], flat_anchor: (f64, f64)) -> Vec<(String, DirFrame)> {
  const ALIASES: [&str; 4] = ["s", "e", "n", "w"];
  let base = frames[VARIANTS_PER_DEF - 1][ROTATIONS_PER_DEF - 1];
  let unauthored = DirFrame { sub: (0.0, 0.0, 1.0, 1.0), anchor: flat_anchor };
  let mut out = Vec::new();
  if base != unauthored {
    out.push(("default".to_string(), base));
  }
  let key_r = |r: usize| ALIASES.get(r).map(|a| (*a).to_string()).unwrap_or_else(|| format!("r{r}"));
  let mut rot: Vec<Option<DirFrame>> = vec![None; ROTATIONS_PER_DEF];
  for r in 0..ROTATIONS_PER_DEF - 1 {
    let cand = frames[VARIANTS_PER_DEF - 1][r];
    if cand != base {
      rot[r] = Some(cand);
      out.push((key_r(r), cand));
    }
  }
  let mut var: Vec<Option<DirFrame>> = vec![None; VARIANTS_PER_DEF];
  for v in 0..VARIANTS_PER_DEF - 1 {
    let cand = frames[v][ROTATIONS_PER_DEF - 1];
    if cand != base {
      var[v] = Some(cand);
      out.push((format!("v{v}"), cand));
    }
  }
  for (v, row) in frames.iter().enumerate() {
    for (r, got) in row.iter().enumerate() {
      let expect = var[v].or(rot[r]).unwrap_or(base);
      if *got != expect {
        out.push((format!("v{v}.{}", key_r(r)), *got));
      }
    }
  }
  out
}

fn part_toml(out: &mut String, table: &str, p: &VisualPart, first: bool, footprint: (f64, f64)) {
  let _ = writeln!(out, "\n  [[{table}.part]]");
  if let Some(t) = &p.texture {
    let _ = writeln!(out, "  texture = \"{t}\"");
  }
  let _ = writeln!(out, "  tint = {}", hex(p.tint));
  if p.geo_color != p.tint {
    let _ = writeln!(out, "  geo = {}", hex(p.geo_color));
  }
  if first && footprint != (1.0, 1.0) {
    let _ = writeln!(out, "  footprint = {{ w = {}, h = {} }}", f(footprint.0), f(footprint.1));
  }
  if p.anchor != (0.5, 0.5) {
    let _ = writeln!(out, "  anchor = {{ x = {}, y = {} }}", f(p.anchor.0), f(p.anchor.1));
  }
  if p.sprite_anchor != (0.5, 0.5) {
    let _ = writeln!(out, "  sprite_anchor = {{ x = {}, y = {} }}", f(p.sprite_anchor.0), f(p.sprite_anchor.1));
  }
  if p.size != 1.0 {
    let _ = writeln!(out, "  size = {}", f(p.size));
  }
  if p.span != 1.0 {
    let _ = writeln!(out, "  span = {}", f(p.span));
  }
  if p.scale != 1.0 {
    let _ = writeln!(out, "  scale = {}", f(p.scale));
  }
  if p.sprite_scale != (1.0, 1.0) {
    let _ = writeln!(out, "  sprite_scale = {{ w = {}, h = {} }}", f(p.sprite_scale.0), f(p.sprite_scale.1));
  }
  if p.part != 0 {
    let _ = writeln!(out, "  part = {}", p.part);
  }
  if p.depth != 0.0 {
    let _ = writeln!(out, "  depth = {}", f(p.depth));
  }
  if p.offset != (0.0, 0.0) || p.elevation != 0.0 {
    let _ = writeln!(
      out,
      "  offset = {{ x = {}, y = {}, z = {} }}",
      f(p.offset.0), f(p.offset.1), f(p.elevation)
    );
  }
  let keys = subframe_keys(&p.dir_frames, p.sprite_anchor);
  if !keys.is_empty() {
    let _ = writeln!(out, "\n  [{table}.part.subframe]");
    for (k, d) in keys {
      let mut fields = vec![
        format!("x = {}", f(d.sub.0)),
        format!("y = {}", f(d.sub.1)),
        format!("w = {}", f(d.sub.2)),
        format!("h = {}", f(d.sub.3)),
      ];
      if d.anchor.0 != p.sprite_anchor.0 {
        fields.push(format!("ax = {}", f(d.anchor.0)));
      }
      if d.anchor.1 != p.sprite_anchor.1 {
        fields.push(format!("ay = {}", f(d.anchor.1)));
      }
      let _ = writeln!(out, "  \"{k}\" = {{ {} }}", fields.join(", "));
    }
  }
}

#[test]
fn convert_the_rd_corpus_to_toml() {
  if std::env::var("CONVERT_TOML").is_err() {
    return;
  }
  let Some(b) = corpus() else { return };
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");

  // ── tiles.toml ──
  let mut out = String::from("# Tiles — see docs/VARIABLES.md § TOML content schema.\n# Ids are STORED DATA (zone kind_references) — never renumber, never reuse (F1).\n");
  for (i, name) in b.tile_names().iter().enumerate() {
    let id = i as u16 + 1;
    let _ = writeln!(out, "\n[[tile]]\nid = {id}\nname = \"{name}\"");
    if let Some(v) = b.visual_for_def(id) {
      if let Some(t) = &v.texture {
        let _ = writeln!(out, "texture = \"{t}\"");
      }
      let _ = writeln!(out, "tint = {}", hex(v.tint));
      if v.geo_color != v.tint {
        let _ = writeln!(out, "geo = {}", hex(v.geo_color));
      }
    }
    if let Some(h) = b.tile_height(id) {
      let _ = writeln!(out, "height = {}", f(h));
    }
    if let Some(build) = b.tile_build(id) {
      let _ = writeln!(out, "build = \"{build}\"");
    }
    let lanes = &b.tile_lighting_lanes()[i * 6..i * 6 + 6];
    if lanes[4] != 0.0 {
      let _ = writeln!(out, "cast_shadow = {}", f(lanes[4]));
    }
    if lanes[5] != 0.0 {
      let _ = writeln!(out, "receives_shadows = {}", f(lanes[5]));
    }
    if lanes[3] != 0.0 {
      let _ = writeln!(out, "rotation = {}", f(lanes[3]));
    }
    if lanes[0] != 0.0 || lanes[1] != 0.0 {
      let _ = writeln!(out, "linked = {{ w = {}, h = {} }}", f(lanes[0]), f(lanes[1]));
    }
    if lanes[2] != 0.0 {
      let _ = writeln!(out, "padding = {}", f(lanes[2]));
    }
    let packed = &b.tile_packed_channels()[i];
    if packed.iter().any(|c| c.material_id != 0 || c.tint != 0) {
      let cells: Vec<String> = packed
        .iter()
        .take_while(|c| c.material_id != 0 || c.tint != 0)
        .map(|c| {
          let mut kv = Vec::new();
          if c.material_id != 0 {
            kv.push(format!("material = \"{}\"", b.material_name(c.material_id).unwrap_or("")));
          }
          kv.push(format!("tint = {}", hex(c.tint)));
          format!("{{ {} }}", kv.join(", "))
        })
        .collect();
      let _ = writeln!(out, "packed = [ {} ]", cells.join(", "));
    }
  }
  std::fs::write(root.join("tiles.toml"), &out).unwrap();

  // ── things.toml ──
  let mut out = String::from("# Things — flora, wall kinds, PAWNS (a pawn is a thing with parts).\n# Ids are STORED DATA (thing entries + packed pawn defs) — never renumber (F1).\n");
  for (i, name) in b.thing_names().iter().enumerate() {
    let id = i as u16 + 1;
    let _ = writeln!(out, "\n[[thing]]\nid = {id}\nname = \"{name}\"");
    if let Some(s) = b.thing_speed(id) {
      let _ = writeln!(out, "speed = {s}");
    }
    let needs = b.thing_needs(id);
    if !needs.is_empty() {
      let names: Vec<String> =
        needs.iter().filter_map(|&n| b.need_name(n)).map(|n| format!("\"{n}\"")).collect();
      let _ = writeln!(out, "needs = [{}]", names.join(", "));
    }
    let Some(v) = b.visual_for_object(id) else { continue };
    if let Some(l) = v.light {
      let mut kv = vec![
        format!("r = {}", f(l.color.0)),
        format!("g = {}", f(l.color.1)),
        format!("b = {}", f(l.color.2)),
        format!("intensity = {}", f(l.intensity)),
        format!("reach = {}", f(l.reach)),
        format!("radius = {}", f(l.radius)),
        format!("height = {}", f(l.height)),
      ];
      if !l.cast {
        kv.push("cast = false".into());
      }
      if l.hot {
        kv.push("hot = true".into());
      }
      if l.flicker {
        kv.push("flicker = true".into());
      }
      let _ = writeln!(out, "light = {{ {} }}", kv.join(", "));
    }
    if v.packed.iter().any(|c| c.material_id != 0 || c.tint != 0) {
      let cells: Vec<String> = v
        .packed
        .iter()
        .take_while(|c| c.material_id != 0 || c.tint != 0)
        .map(|c| {
          let mut kv = Vec::new();
          if c.material_id != 0 {
            kv.push(format!("material = \"{}\"", b.material_name(c.material_id).unwrap_or("")));
          }
          kv.push(format!("tint = {}", hex(c.tint)));
          format!("{{ {} }}", kv.join(", "))
        })
        .collect();
      let _ = writeln!(out, "packed = [ {} ]", cells.join(", "));
    }
    for (pi, p) in v.parts.iter().enumerate() {
      part_toml(&mut out, "thing", p, pi == 0, v.footprint);
    }
  }
  std::fs::write(root.join("things.toml"), &out).unwrap();

  // ── materials.toml ──
  let mut out = String::from("# Materials — the render registry (hue/chroma jitter; never lightness).\n");
  for (i, name) in b.material_names().iter().enumerate() {
    let p = &b.material_params_all()[i];
    let _ = writeln!(out, "\n[[material]]\nid = {}\nname = \"{name}\"", i + 1);
    if !p.noise_field.is_empty() {
      let _ = writeln!(out, "noise_field = \"{}\"", p.noise_field);
    }
    if p.hue_swing != 0.0 {
      let _ = writeln!(out, "hue_swing = {}", f(p.hue_swing));
    }
    if p.chroma_swing != 0.0 {
      let _ = writeln!(out, "chroma_swing = {}", f(p.chroma_swing));
    }
    if p.warm_cool_bias != 0.0 {
      let _ = writeln!(out, "warm_cool_bias = {}", f(p.warm_cool_bias));
    }
    if p.sample_space != "uv" {
      let _ = writeln!(out, "sample_space = \"{}\"", p.sample_space);
    }
    if !p.detail_field.is_empty() {
      let _ = writeln!(
        out,
        "detail = {{ field = \"{}\", amp = {}, scale = {} }}",
        p.detail_field, f(p.detail_amp), f(p.detail_scale)
      );
    }
  }
  std::fs::write(root.join("materials.toml"), &out).unwrap();

  // ── needs.toml (needs + moodlets — one model, one file) ──
  let mut out = String::from("# Needs & moodlets — hidden satisfactions, displayed consequences.\n# Ids ride inside pawn PAYLOAD words — never renumber (F1).\n");
  for (i, name) in b.need_names().iter().enumerate() {
    let p = &b.need_params_all()[i];
    let _ = writeln!(out, "\n[[need]]\nid = {}\nname = \"{name}\"", i + 1);
    if p.label != *name {
      let _ = writeln!(out, "label = \"{}\"", p.label);
    }
    if p.deplete != 0.0 {
      let _ = writeln!(out, "deplete = {}", f(p.deplete));
    }
    if !p.bands.is_empty() {
      let cells: Vec<String> = p
        .bands
        .iter()
        .map(|band| format!("{{ moodlet = \"{}\", lo = {}, hi = {} }}", band.moodlet, f(band.lo), f(band.hi)))
        .collect();
      let _ = writeln!(out, "band = [\n  {},\n]", cells.join(",\n  "));
    }
  }
  for (i, name) in b.moodlet_names().iter().enumerate() {
    let p = &b.moodlet_params_all()[i];
    let _ = writeln!(out, "\n[[moodlet]]\nid = {}\nname = \"{name}\"", i + 1);
    if p.label != *name {
      let _ = writeln!(out, "label = \"{}\"", p.label);
    }
    if p.mood != 0.0 {
      let _ = writeln!(out, "mood = {}", f(p.mood));
    }
    if p.duration != 0.0 {
      let _ = writeln!(out, "duration = {}", f(p.duration));
    }
  }
  std::fs::write(root.join("needs.toml"), &out).unwrap();

  eprintln!("converted: tiles/things/materials/needs .toml written (biomes are hand-translated)");
}
