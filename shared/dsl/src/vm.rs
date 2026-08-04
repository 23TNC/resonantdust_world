//! The VM — execute a hook's code body against an in-memory [`Store`].
//!
//! A trimmed descendant of the old game's interpreter: same value model (a tree
//! of [`Cell`]s addressed by dotted/colon paths), the same postfix stack
//! discipline, and the same prim-handle mechanism, but only the ops the current
//! content exercises. A tile's `@on_create` hook builds a visual primitive and
//! configures it through a handle:
//!
//! ```text
//! "tile ^prim call &tile export    ; make a `tile` prim, EXPORT its handle as `tile`
//! "white &tile.texture set          ; write through the handle to the prim
//! #4b573e &tile.tint set
//! 0 return
//! ```
//!
//! So the machine needs: literals, slot writes (`set`), exported writes
//! (`export`), value reads, arithmetic + comparisons, `if`/`!if` guards, the
//! `^system` / `call` FFI for prim construction, `return`, and the reactive
//! pair `dirty` (re-schedule this object's `on_update`) + `date` (the
//! runtime-stamped clock, for `dt`). The `prims` array the hook builds is the
//! output the client renderer reads.

use crate::parser::{Stmt, Token};

/// A stored slot. Maps are insertion-ordered, so a numeric path segment indexes
/// an `Arr` positionally and a name indexes a `Map` by key. Mirrors the old
/// game's `Cell` (minus the recipe-only variants).
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
  Int(i64),
  Float(f64),
  Sym(String),
  Map(Vec<(String, Cell)>),
  Arr(Vec<Cell>),
  /// A symlink to another store path (`prims.0`). The handle `^prim call`
  /// returns: the prim is appended to `prims` and a `Ref` to it handed back, so
  /// `&handle.tint set` writes THROUGH to that prim (see [`Store::follow_refs`]).
  /// A terminal handle (`&handle export`, `*handle`) is left as-is.
  Ref(String),
}

impl Cell {
  /// Numeric value as i64 — `Int` exact, `Float` truncated, everything else 0.
  pub fn as_int(&self) -> i64 {
    match self {
      Cell::Int(n) => *n,
      Cell::Float(f) => *f as i64,
      _ => 0,
    }
  }
  /// Numeric value as f64 — `Float` exact, `Int` widened, everything else 0.
  pub fn as_f64(&self) -> f64 {
    match self {
      Cell::Int(n) => *n as f64,
      Cell::Float(f) => *f,
      _ => 0.0,
    }
  }
}

// ---------- Path addressing ----------

/// One step of a parsed path: a literal name (an `Arr` index when all-digits,
/// else a `Map` key).
#[derive(Debug, Clone, PartialEq)]
enum Seg {
  Lit(String),
  Idx(usize),
}

/// A bare segment: an `Idx` when all-digits, else a `Lit` key.
fn seg_of(s: &str) -> Seg {
  if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
    Seg::Idx(s.parse().unwrap_or(0))
  } else {
    Seg::Lit(s.to_string())
  }
}

/// Parse a path with no `*`-interpolation (e.g. a `Ref`'s `"prims.0"` target).
fn parse_segs(path: &str) -> Vec<Seg> {
  path.split([':', '.']).filter(|s| !s.is_empty()).map(seg_of).collect()
}

/// Split a path on `:`/`.` into segments. A `*name` segment interpolates: it
/// reads `name` from the local store and uses the result as the next step (an
/// `Int` → array index, a `Sym` → map key) — the same indirection the old VM
/// used for `objects.*var.0`. Plain segments pass through verbatim.
fn parse_path(root: &Cell, path: &str) -> Vec<Seg> {
  let raw: Vec<&str> = path.split([':', '.']).filter(|s| !s.is_empty()).collect();
  let mut out = Vec::new();
  let mut i = 0;
  while i < raw.len() {
    if let Some(name) = raw[i].strip_prefix('*') {
      let mut sub = name.to_string();
      i += 1;
      while i < raw.len() && !raw[i].is_empty() && raw[i].bytes().all(|b| b.is_ascii_digit()) {
        sub.push('.');
        sub.push_str(raw[i]);
        i += 1;
      }
      match walk_read(root, &parse_segs(&sub)) {
        Some(Cell::Sym(s)) => out.push(Seg::Lit(s.clone())),
        Some(c) => out.push(Seg::Idx(c.as_int().max(0) as usize)),
        None => out.push(Seg::Idx(0)),
      }
    } else {
      out.push(seg_of(raw[i]));
      i += 1;
    }
  }
  out
}

fn step<'a>(cur: &'a Cell, seg: &Seg) -> Option<&'a Cell> {
  match (cur, seg) {
    (Cell::Arr(v), Seg::Idx(i)) => v.get(*i),
    (Cell::Map(m), Seg::Idx(i)) => m.get(*i).map(|(_, c)| c),
    (Cell::Arr(v), Seg::Lit(s)) => s.parse::<usize>().ok().and_then(|i| v.get(i)),
    (Cell::Map(m), Seg::Lit(s)) => m.iter().find(|(k, _)| k == s).map(|(_, c)| c),
    _ => None,
  }
}

fn walk_read<'a>(cur: &'a Cell, segs: &[Seg]) -> Option<&'a Cell> {
  let Some((head, rest)) = segs.split_first() else { return Some(cur) };
  step(cur, head).and_then(|c| walk_read(c, rest))
}

fn walk_write(cur: &mut Cell, segs: &[Seg], val: Cell) {
  let Some((head, rest)) = segs.split_first() else {
    *cur = val;
    return;
  };
  match head {
    Seg::Idx(i) => {
      // A numeric step builds (or grows) an array — unless `cur` is already a
      // map, in which case the index addresses a map slot positionally.
      if !matches!(cur, Cell::Arr(_) | Cell::Map(_)) {
        *cur = Cell::Arr(Vec::new());
      }
      match cur {
        Cell::Arr(v) => {
          if *i >= v.len() {
            v.resize(i + 1, Cell::Int(0));
          }
          walk_write(&mut v[*i], rest, val);
        }
        Cell::Map(m) if *i < m.len() => walk_write(&mut m[*i].1, rest, val),
        _ => {}
      }
    }
    Seg::Lit(s) => {
      if !matches!(cur, Cell::Map(_)) {
        *cur = Cell::Map(Vec::new());
      }
      if let Cell::Map(m) = cur {
        match m.iter().position(|(k, _)| k == s) {
          Some(idx) => walk_write(&mut m[idx].1, rest, val),
          None => {
            m.push((s.clone(), Cell::Int(0)));
            let last = m.len() - 1;
            walk_write(&mut m[last].1, rest, val);
          }
        }
      }
    }
  }
}

/// The in-memory slot tree a hook reads and writes. Slots are addressed by
/// dotted/colon paths (`tile.tint`), auto-vivifying maps/arrays as a write
/// descends, and following `Ref` handles so a write through `&tile.tint` lands
/// in the prim the handle points at. Also records which slots were `export`ed.
#[derive(Clone, Debug, PartialEq)]
pub struct Store {
  root: Cell,
  /// Slot names written with `export` (in first-export order, deduped). The
  /// object's persistent / cross-scope variables — recorded here for the runtime
  /// to act on; the VM itself just notes them.
  exports: Vec<String>,
  /// Set by the `dirty` op: the body asked to be re-run (an `on_update` that
  /// re-schedules itself — the self-driven "tic"). The runtime checks this after
  /// a hook and re-enqueues the object if set. NOT touched by ordinary writes —
  /// re-running `on_update` is explicit, so a hook never loops by accident.
  dirty: bool,
  /// The wall-clock time (ms) the runtime stamps before running a hook; the
  /// `date` op reads it. Injected (not sampled in the VM) so the machine stays
  /// pure and wasm-safe — and so a deferred/budgeted update still gets the real
  /// elapsed `dt` between passes. `0` until the runtime sets it.
  now_ms: i64,
  /// The biome dimensions the runtime samples for this tile before running a
  /// `@define` / `@on_create` biome hook: `[dim0, dim1, …, dimN]` in `[0, 1]`
  /// (temperature, humidity, elevation, …). The `^biome call` op reads them as a
  /// `Cell::Arr`, so content does `^biome call &biome set` then `*biome.0`.
  /// Injected (not sampled in the VM) so the machine stays deterministic and
  /// wasm-safe — worldgen samples the noise, the DSL only classifies. Empty until
  /// the runtime sets it.
  biome: Vec<f64>,
  /// The per-tile RNG seed the runtime stamps before a biome hook. The `^rand`
  /// op hashes it with a caller-supplied salt to a deterministic `[0, 1)` float,
  /// so a biome can scatter independent features (trees on salt 1, flora on salt
  /// 2, …) that reproduce on a re-seed. `0` until the runtime sets it.
  seed: u64,
}

impl Default for Store {
  fn default() -> Self {
    Store {
      root: Cell::Map(Vec::new()),
      exports: Vec::new(),
      dirty: false,
      now_ms: 0,
      biome: Vec::new(),
      seed: 0,
    }
  }
}

impl Store {
  /// Consume the store, yielding its root cell.
  pub fn into_root(self) -> Cell {
    self.root
  }
  /// The slot names written with `export`, in first-export order.
  pub fn exports(&self) -> &[String] {
    &self.exports
  }
  /// Whether the body asked to be re-run (the `dirty` op fired). The runtime
  /// reads this after `on_update` to decide whether to re-enqueue the object.
  pub fn is_dirty(&self) -> bool {
    self.dirty
  }
  /// Clear the dirty flag — the runtime calls this before each `on_update` pass,
  /// so each pass decides afresh whether to keep going.
  pub fn clear_dirty(&mut self) {
    self.dirty = false;
  }
  /// Stamp the wall-clock time (ms) the next hook will read via `date`. The
  /// runtime sets this per pass; `dt` is the difference the content computes.
  pub fn set_now(&mut self, now_ms: i64) {
    self.now_ms = now_ms;
  }
  /// Inject this tile's biome dimensions (what `^biome call` returns). Worldgen
  /// samples the noise fields and stamps them before running a biome hook.
  pub fn set_biome(&mut self, dims: Vec<f64>) {
    self.biome = dims;
  }
  /// The injected biome dimensions (empty until [`set_biome`](Self::set_biome)).
  pub fn biome(&self) -> &[f64] {
    &self.biome
  }
  /// Inject this tile's RNG seed (what `^rand` hashes). Worldgen derives it from
  /// the tile's world coordinates so scatter is deterministic and reproducible.
  pub fn set_seed(&mut self, seed: u64) {
    self.seed = seed;
  }
  /// A deterministic `[0, 1)` float from the tile seed and a caller `salt` — the
  /// engine side of `<salt> ^rand call`. Different salts give independent draws
  /// for the same tile (SplitMix64 finalizer over `seed ^ mix(salt)`), so flora
  /// and trees scatter without correlating. Pure of `self` (no interior state),
  /// so re-running a hook reproduces the same draws.
  pub fn rand(&self, salt: i64) -> f64 {
    // ONE derivation (toml-content P0): the TOML rule classifier draws through the
    // same function, so no scatter re-rolls across the dialect migration.
    crate::loader::tile_rand(self.seed, salt)
  }
  /// Read a slot (`None` if the path doesn't resolve), following `Ref` handles.
  pub fn read(&self, path: &str) -> Option<&Cell> {
    let segs = self.follow_refs(parse_path(&self.root, path));
    walk_read(&self.root, &segs)
  }
  /// Write a slot, building intermediate maps/arrays and following `Ref` handles.
  pub fn write(&mut self, path: &str, val: Cell) {
    let segs = self.follow_refs(parse_path(&self.root, path));
    walk_write(&mut self.root, &segs, val);
  }

  /// Record `key` as an exported slot (deduped).
  fn mark_export(&mut self, key: &str) {
    if !self.exports.iter().any(|k| k == key) {
      self.exports.push(key.to_string());
    }
  }

  /// Expand `Ref` symlinks in a parsed path: if a proper PREFIX resolves to a
  /// `Ref(p)` and there's a tail, splice `p`'s segments in front of the tail and
  /// repeat. So `tile.tint` (where `tile` holds `Ref("prims.0")`) resolves to
  /// `prims.0.tint`. A terminal `Ref` (no tail) is left alone — that's the handle
  /// itself (`&tile export` overwrites it; `*tile` reads it).
  fn follow_refs(&self, segs: Vec<Seg>) -> Vec<Seg> {
    let mut segs = segs;
    loop {
      let mut spliced = false;
      for i in 1..segs.len() {
        if let Some(Cell::Ref(p)) = walk_read(&self.root, &segs[..i]) {
          let mut next = parse_segs(p);
          next.extend_from_slice(&segs[i..]);
          segs = next;
          spliced = true;
          break;
        }
      }
      if !spliced {
        return segs;
      }
    }
  }

  /// Append a `{kind}` prim to the `prims` array (creating it) and return its
  /// index — the engine side of `^prim call`. The caller wraps the index in a
  /// `Ref` handle the DSL then configures. `prims` is the renderer's input.
  fn prims_push(&mut self, kind: String) -> usize {
    if !matches!(&self.root, Cell::Map(_)) {
      self.root = Cell::Map(Vec::new());
    }
    let Cell::Map(root) = &mut self.root else { unreachable!() };
    let entry = match root.iter_mut().find(|(k, _)| k == "prims") {
      Some((_, c)) => c,
      None => {
        root.push(("prims".into(), Cell::Arr(Vec::new())));
        &mut root.last_mut().unwrap().1
      }
    };
    if !matches!(entry, Cell::Arr(_)) {
      *entry = Cell::Arr(Vec::new());
    }
    let Cell::Arr(v) = entry else { return 0 };
    v.push(Cell::Map(vec![("kind".into(), Cell::Sym(kind))]));
    v.len() - 1
  }
}

// ---------- Execution ----------

/// A value on the postfix operand stack while a line evaluates. `Addr` is a slot
/// reference (`&path`) awaiting a verb; `Sys` is a pending `^system` call; `Cell`
/// carries a built value (e.g. a prim `Ref`); the rest are plain values.
#[derive(Clone, Debug)]
enum Item {
  Val(i64),
  Float(f64),
  Sym(String),
  Addr(String),
  Sys(String),
  Cell(Cell),
}

impl Item {
  fn int(&self) -> i64 {
    match self {
      Item::Val(n) => *n,
      Item::Float(f) => *f as i64,
      Item::Cell(c) => c.as_int(),
      _ => 0,
    }
  }
  fn f64(&self) -> f64 {
    match self {
      Item::Val(n) => *n as f64,
      Item::Float(f) => *f,
      Item::Cell(c) => c.as_f64(),
      _ => 0.0,
    }
  }
  fn is_float(&self) -> bool {
    matches!(self, Item::Float(_) | Item::Cell(Cell::Float(_)))
  }
  fn into_cell(self) -> Cell {
    match self {
      Item::Sym(s) => Cell::Sym(s),
      Item::Float(f) => Cell::Float(f),
      Item::Cell(c) => c,
      other => Cell::Int(other.int()),
    }
  }
}

/// Guard against a runaway body (no loops today, but cheap insurance).
const STEP_CAP: u32 = 100_000;

/// Run a hook body against `store`, returning the body's return value
/// (`<val> return`, or `0` on fall-through). Each line evaluates postfix on its
/// own operand stack; `set`/`export` write slots, `return` halts. Mutations land
/// in `store`, which the caller reads afterward (e.g. the `prims` it built).
pub fn run(body: &[Stmt], store: &mut Store) -> Result<i64, String> {
  let mut steps = 0u32;
  for stmt in body {
    let toks = match stmt {
      // Labels are jump targets; with no `goto` yet they're inert markers.
      Stmt::LabelDef(_) => continue,
      Stmt::Instr(t) => t,
    };
    steps += 1;
    if steps > STEP_CAP {
      return Err("step cap exceeded".into());
    }

    let mut st: Vec<Item> = Vec::new();
    'line: for tok in toks {
      match tok {
        Token::Number(n) => st.push(Item::Val(*n)),
        Token::Float(f) => st.push(Item::Float(*f)),
        // `#rrggbb` → the packed 0xRRGGBB integer (drops the `#`).
        Token::Color(s) => {
          st.push(Item::Val(i64::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0)));
        }
        // `"text` → a literal string symbol, verbatim.
        Token::Str(s) => st.push(Item::Sym(s.clone())),
        // `&path` → a pending slot address.
        Token::Slot(s) => st.push(Item::Addr(s.clone())),
        // `^name` → a pending system call, resolved by `call`.
        Token::System(s) => st.push(Item::Sys(s.clone())),
        // `*path` → read the slot's current value (following handles).
        Token::Value(s) => match store.read(s) {
          Some(Cell::Sym(sym)) => st.push(Item::Sym(sym.clone())),
          Some(Cell::Float(f)) => st.push(Item::Float(*f)),
          Some(c @ (Cell::Ref(_) | Cell::Map(_) | Cell::Arr(_))) => st.push(Item::Cell(c.clone())),
          Some(c) => st.push(Item::Val(c.as_int())),
          None => st.push(Item::Val(0)),
        },
        // A bare word is an op (or, lacking a meaning, a literal symbol — so a
        // content constant like `tile` pushes itself for a following `set`).
        Token::Word(w) => match w.as_str() {
          "set" => {
            let addr = st.pop().ok_or("set: empty stack (address)")?;
            let val = st.pop().ok_or("set: empty stack (value)")?;
            let Item::Addr(a) = addr else { return Err("set: target is not a &slot".into()) };
            store.write(&a, val.into_cell());
          }
          // `<value> &slot export` — `set`, plus flag the slot as an exported
          // variable (persistent on the object, visible to other scopes, and
          // dirty-marking when changed). The VM records the name; the runtime
          // acts on it.
          "export" => {
            let addr = st.pop().ok_or("export: empty stack (address)")?;
            let val = st.pop().ok_or("export: empty stack (value)")?;
            let Item::Addr(a) = addr else { return Err("export: target is not a &slot".into()) };
            store.write(&a, val.into_cell());
            store.mark_export(&a);
          }
          // `<args…> ^system call` / `&handle.method call`.
          "call" => match st.pop() {
            // `^prim` — construct a primitive of the popped kind (`"tile`), push
            // it onto `prims`, and return a `Ref` handle the DSL configures.
            Some(Item::Sys(name)) if name == "prim" => {
              let kind = match st.pop() {
                Some(Item::Sym(s)) => s,
                _ => return Err("^prim: expected a kind string (e.g. \"tile)".into()),
              };
              let idx = store.prims_push(kind);
              st.push(Item::Cell(Cell::Ref(format!("prims.{idx}"))));
            }
            // `^biome call` — the injected biome dimensions as a `Cell::Arr` of
            // floats. Content does `^biome call &biome set`, then reads a
            // dimension with `*biome.0` / `*biome.1` / … in a later line.
            Some(Item::Sys(name)) if name == "biome" => {
              let dims: Vec<Cell> = store.biome.iter().map(|&f| Cell::Float(f)).collect();
              st.push(Item::Cell(Cell::Arr(dims)));
            }
            // `<salt> ^rand call` — a deterministic `[0, 1)` draw for this tile,
            // salted by the popped int. Lets a biome scatter independent features
            // (`1 ^rand call` for trees, `2 ^rand call` for flora, …).
            Some(Item::Sys(name)) if name == "rand" => {
              let salt = st.pop().map(|i| i.int()).unwrap_or(0);
              st.push(Item::Float(store.rand(salt)));
            }
            // An unknown `^system` — no host is wired into this VM yet, so it
            // yields 0 rather than failing the body.
            Some(Item::Sys(_)) => st.push(Item::Val(0)),
            // `&handle.method call` (e.g. `&tile.destroy call`) — a method on a
            // prim handle. The renderer owns the actual effect (it reads `prims`
            // and acts); at the VM level this is a no-op yielding 0.
            Some(Item::Addr(_)) => st.push(Item::Val(0)),
            other => {
              if let Some(it) = other {
                st.push(it);
              }
            }
          },
          "drop" => {
            st.pop();
          }
          "return" => {
            return Ok(st.pop().map(|i| i.int()).unwrap_or(0));
          }
          "add" | "sub" | "mul" | "div" | "mod" => {
            let b = st.pop().ok_or("arith: empty stack")?;
            let a = st.pop().ok_or("arith: empty stack")?;
            if a.is_float() || b.is_float() {
              let (x, y) = (a.f64(), b.f64());
              st.push(Item::Float(match w.as_str() {
                "add" => x + y,
                "sub" => x - y,
                "mul" => x * y,
                "div" => if y == 0.0 { 0.0 } else { x / y },
                _ => if y == 0.0 { 0.0 } else { x % y },
              }));
            } else {
              let (x, y) = (a.int(), b.int());
              st.push(Item::Val(match w.as_str() {
                "add" => x + y,
                "sub" => x - y,
                "mul" => x * y,
                "div" => if y == 0 { 0 } else { x / y },
                _ => if y == 0 { 0 } else { x % y },
              }));
            }
          }
          "inc" | "dec" => {
            let addr = st.pop().ok_or("inc/dec: empty stack")?;
            let Item::Addr(a) = addr else { return Err("inc/dec: target is not a &slot".into()) };
            let cur = store.read(&a).map(Cell::as_int).unwrap_or(0);
            store.write(&a, Cell::Int(if w == "inc" { cur + 1 } else { cur - 1 }));
          }
          // Comparisons: `<a> <b> <op>` → `1`/`0`. Float-aware (either operand a
          // float picks the float compare); otherwise integer.
          "eq" | "ne" | "lt" | "le" | "gt" | "ge" => {
            let b = st.pop().ok_or("cmp: empty stack")?;
            let a = st.pop().ok_or("cmp: empty stack")?;
            let r = if a.is_float() || b.is_float() {
              let (x, y) = (a.f64(), b.f64());
              match w.as_str() {
                "eq" => x == y,
                "ne" => x != y,
                "lt" => x < y,
                "le" => x <= y,
                "gt" => x > y,
                _ => x >= y,
              }
            } else {
              let (x, y) = (a.int(), b.int());
              match w.as_str() {
                "eq" => x == y,
                "ne" => x != y,
                "lt" => x < y,
                "le" => x <= y,
                "gt" => x > y,
                _ => x >= y,
              }
            };
            st.push(Item::Val(r as i64));
          }
          // Boolean combinators over truthiness (non-zero = true), so a biome
          // `@define` can AND/OR several dimension tests into one predicate line:
          // `*biome.0 0.5 ge *biome.1 0.4 ge and return`.
          "and" | "or" => {
            let b = st.pop().map(|i| i.int()).unwrap_or(0) != 0;
            let a = st.pop().map(|i| i.int()).unwrap_or(0) != 0;
            let r = if w == "and" { a && b } else { a || b };
            st.push(Item::Val(r as i64));
          }
          "not" => {
            let a = st.pop().map(|i| i.int()).unwrap_or(0) != 0;
            st.push(Item::Val((!a) as i64));
          }
          // `<cond> if <rest…>` runs the rest of the LINE only when `cond` is
          // truthy (non-zero); `!if` inverts. A false guard skips to the next
          // line — so `*progress 0 gt if dirty` only re-dirties while progress > 0.
          "if" | "!if" => {
            let c = st.pop().map(|i| i.int()).unwrap_or(0) != 0;
            let go = if w == "if" { c } else { !c };
            if !go {
              break 'line;
            }
          }
          // `dirty` — mark the object for another `on_update` pass. The self-
          // driven "tic": a hook re-schedules itself (often guarded by `if`).
          "dirty" => store.dirty = true,
          // `date` — push the runtime-stamped wall-clock time (ms). Difference two
          // reads across passes for `dt`. Wall-clock, so non-deterministic: keep
          // it to client `:visual` hooks (the server drives time deterministically).
          "date" => st.push(Item::Val(store.now_ms)),
          // An unknown bare word is a literal symbol (content constant).
          other => st.push(Item::Sym(other.to_string())),
        },
        // Unsupported in the current op set; ignore so a body can carry markers.
        Token::Const(_) | Token::Label(_) => {}
      }
    }
  }
  Ok(0)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
  use super::*;
  use crate::parser::parse;

  /// Parse a bare hook body (`@on_create> …`) and run it, returning the store.
  fn run_body(src: &str) -> (Store, i64) {
    let wrapped = format!("@on_create>\n{src}");
    let root = parse(&wrapped).unwrap();
    let body = &root.children[0].body;
    let mut store = Store::default();
    let ret = run(body, &mut store).unwrap();
    (store, ret)
  }

  #[test]
  fn set_writes_a_nested_slot() {
    let (store, ret) = run_body("  #4b573e &visual.color.bg set\n  0 return\n");
    assert_eq!(store.read("visual.color.bg"), Some(&Cell::Int(0x4b573e)));
    assert_eq!(ret, 0);
  }

  #[test]
  fn return_yields_top_of_stack() {
    let (_, ret) = run_body("  7 return\n");
    assert_eq!(ret, 7);
  }

  #[test]
  fn ret_is_no_longer_an_alias_for_return() {
    // `ret` is a retired keyword: it's now just a bare-word symbol, NOT a return,
    // so the body falls through to 0 instead of returning the 7.
    let (_, ret) = run_body("  7 ret\n");
    assert_eq!(ret, 0);
  }

  #[test]
  fn bare_word_is_a_symbol_literal() {
    let (store, _) = run_body("  tile &data.type set\n  0 return\n");
    assert_eq!(store.read("data.type"), Some(&Cell::Sym("tile".into())));
  }

  #[test]
  fn arithmetic_and_value_read() {
    let (store, _) = run_body("  2 3 add &data.n set\n  *data.n 1 add &data.m set\n");
    assert_eq!(store.read("data.n"), Some(&Cell::Int(5)));
    assert_eq!(store.read("data.m"), Some(&Cell::Int(6)));
  }

  #[test]
  fn inc_builds_from_zero() {
    let (store, _) = run_body("  &data.k inc\n  &data.k inc\n");
    assert_eq!(store.read("data.k"), Some(&Cell::Int(2)));
  }

  #[test]
  fn prim_handle_construct_export_and_write_through() {
    // The grass tile's @on_create: build a `tile` prim, export the handle, then
    // configure the prim through the handle.
    let (store, _) = run_body(
      "  \"tile ^prim call &tile export\n  \"white &tile.texture set\n  #4b573e &tile.tint set\n  0 return\n",
    );
    // the handle is a Ref into prims, and it was exported
    assert_eq!(store.read("tile"), Some(&Cell::Ref("prims.0".into())));
    assert_eq!(store.exports(), &["tile".to_string()]);
    // writes through the handle landed on the prim
    assert_eq!(store.read("prims.0.kind"), Some(&Cell::Sym("tile".into())));
    assert_eq!(store.read("prims.0.texture"), Some(&Cell::Sym("white".into())));
    assert_eq!(store.read("prims.0.tint"), Some(&Cell::Int(0x4b573e)));
    // and reading through the handle agrees
    assert_eq!(store.read("tile.tint"), Some(&Cell::Int(0x4b573e)));
  }

  #[test]
  fn destroy_method_call_is_a_safe_noop() {
    // @on_destroy's `&tile.destroy call drop` must run without error (the
    // renderer owns the real effect).
    let (_, ret) = run_body(
      "  \"tile ^prim call &tile export\n  &tile.destroy call drop\n  0 return\n",
    );
    assert_eq!(ret, 0);
  }

  /// Run a body against a caller-supplied store (so a test can seed state and
  /// re-run, the way the runtime drives a persistent per-object store).
  fn run_into(store: &mut Store, src: &str) -> i64 {
    let wrapped = format!("@on_update>\n{src}");
    let body = &parse(&wrapped).unwrap().children[0].body;
    run(body, store).unwrap()
  }

  #[test]
  fn comparisons_and_if_guard() {
    // `gt` then `if`: the guarded write happens only when the predicate holds.
    let (store, _) = run_body("  5 3 gt if 1 &data.hit set\n  0 return\n");
    assert_eq!(store.read("data.hit"), Some(&Cell::Int(1)));
    let (store, _) = run_body("  2 3 gt if 1 &data.hit set\n  0 return\n");
    assert_eq!(store.read("data.hit"), None); // guard was false → write skipped
  }

  #[test]
  fn dirty_is_only_set_by_the_dirty_op() {
    // an ordinary write does NOT dirty — re-running on_update is explicit.
    let (plain, _) = run_body("  #ff00ff &tile.tint set\n  0 return\n");
    assert!(!plain.is_dirty());
    // the `dirty` op sets it.
    let (d, _) = run_body("  dirty\n  0 return\n");
    assert!(d.is_dirty());
  }

  #[test]
  fn self_dirtying_countdown_runs_then_quiesces() {
    // The "tic" expressed as a self-re-dirtying update: decrement a counter and
    // re-dirty while it's positive. Drive it the way the runtime would — same
    // store, clearing dirty before each pass.
    let mut store = Store::default();
    store.write("progress", Cell::Int(2));
    let body = "  &progress dec\n  *progress 0 gt if dirty\n  0 return\n";

    store.clear_dirty();
    run_into(&mut store, body);
    assert_eq!(store.read("progress"), Some(&Cell::Int(1)));
    assert!(store.is_dirty(), "still counting → re-dirties");

    store.clear_dirty();
    run_into(&mut store, body);
    assert_eq!(store.read("progress"), Some(&Cell::Int(0)));
    assert!(!store.is_dirty(), "hit zero → goes quiet (no re-dirty)");
  }

  #[test]
  fn biome_call_returns_the_injected_dimensions() {
    // `^biome call &biome set` stores the injected dims as an array; `*biome.N`
    // reads a dimension in a later line (the operand stack is per-line).
    let src = "@on_create>\n  ^biome call &biome set\n  *biome.0 &t set\n  *biome.2 &e set\n  0 return\n";
    let body = &parse(src).unwrap().children[0].body;
    let mut store = Store::default();
    store.set_biome(vec![0.25, 0.5, 0.9]);
    run(body, &mut store).unwrap();
    assert_eq!(store.read("t"), Some(&Cell::Float(0.25)));
    assert_eq!(store.read("e"), Some(&Cell::Float(0.9)));
  }

  #[test]
  fn rand_is_deterministic_and_salt_varies() {
    // Same seed + salt → same draw (reproducible scatter); different salt → an
    // independent draw; every draw is in [0, 1).
    let a = Store { seed: 12345, ..Store::default() };
    assert_eq!(a.rand(1), a.rand(1), "same seed+salt reproduces");
    assert_ne!(a.rand(1), a.rand(2), "salt decorrelates");
    for salt in 0..64 {
      let r = a.rand(salt);
      assert!((0.0..1.0).contains(&r), "draw {r} out of range");
    }
    // A different tile seed gives a different sequence.
    let b = Store { seed: 999, ..Store::default() };
    assert_ne!(a.rand(1), b.rand(1));
  }

  #[test]
  fn rand_scatter_line_places_a_thing_below_threshold() {
    // The forest-style scatter line: draw, compare, guard a `set`. Pick a seed
    // whose salt-1 draw is small so the guard fires.
    let src = "@on_create>\n  grass &tile set\n  1 ^rand call 0.999 lt if tree &thing.1 set\n  0 return\n";
    let body = &parse(src).unwrap().children[0].body;
    let mut store = Store::default();
    store.set_seed(42);
    run(body, &mut store).unwrap();
    assert_eq!(store.read("tile"), Some(&Cell::Sym("grass".into())));
    assert_eq!(store.read("thing.1"), Some(&Cell::Sym("tree".into())));
  }

  #[test]
  fn boolean_combinators_and_or_not() {
    // `and` / `or` / `not` over truthiness, one predicate line at a time.
    let (s, _) = run_body("  1 1 and &a set\n  1 0 and &b set\n  1 0 or &c set\n  0 0 or &d set\n  0 not &e set\n  0 return\n");
    assert_eq!(s.read("a"), Some(&Cell::Int(1)));
    assert_eq!(s.read("b"), Some(&Cell::Int(0)));
    assert_eq!(s.read("c"), Some(&Cell::Int(1)));
    assert_eq!(s.read("d"), Some(&Cell::Int(0)));
    assert_eq!(s.read("e"), Some(&Cell::Int(1)));
  }

  #[test]
  fn define_style_predicate_is_one_line() {
    // A biome `@define`: stash dims in a slot, then AND several threshold tests
    // into one line and `return` the verdict.
    let src = "@define>\n  ^biome call &b set\n  *b.2 0.42 ge *b.0 0.35 ge and *b.1 0.55 ge and return\n";
    let body = &parse(src).unwrap().children[0].body;
    let pass = {
      let mut s = Store::default();
      s.set_biome(vec![0.5, 0.7, 0.6]); // temperate, humid, land
      run(body, &mut s).unwrap()
    };
    let fail = {
      let mut s = Store::default();
      s.set_biome(vec![0.5, 0.7, 0.3]); // underwater → below the elevation gate
      run(body, &mut s).unwrap()
    };
    assert_eq!(pass, 1);
    assert_eq!(fail, 0);
  }

  #[test]
  fn date_reads_the_injected_clock_for_dt() {
    // `date` returns the runtime-stamped time; differencing two stamps gives dt.
    let mut store = Store::default();
    store.write("t.1", Cell::Int(1_000)); // previous timestamp
    store.set_now(1_016); // runtime stamps this pass at +16ms
    run_into(&mut store, "  date &t.2 set\n  *t.2 *t.1 sub &dt set\n  0 return\n");
    assert_eq!(store.read("t.2"), Some(&Cell::Int(1_016)));
    assert_eq!(store.read("dt"), Some(&Cell::Int(16)));
  }
}
