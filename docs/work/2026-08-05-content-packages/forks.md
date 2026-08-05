# Forks — content packages

## F1 — a package is a FOLDER; there is no manifest {#f1}

_2026-08-05._ `content/**/*.toml` is the corpus. Drop `content/mods/foo/` in and its defs load. No
package manifest, no registration list, no declared load order.

**Chosen** because every reason the corpus used to need structure has already been removed by the
streams before this one:

- ids come from the **registry**, not corpus position, so file order cannot renumber anything;
- the taxonomy is **authored**, so a def's identity does not depend on which file holds it;
- `(taxonomy, version)` uniqueness is enforced at load, so two packages claiming one definition is a
  **load error by construction** rather than a silent last-writer-wins.

A manifest would add a second place recording which files exist — the exact thing
[`content-toml-only` F4](../2026-08-04-content-toml-only/forks.md#f4) deleted `manifest.json` for,
and which went stale within one stream of being written.

Rejected: **a per-package `package.toml`** (a second index of the filesystem, and nothing would read
it that `read_dir` does not already answer); **an explicit load-order file** (order is meaningless
once ids come from the registry — and if it ever stops being meaningless, that is
[I3](issues.md#i3)'s conversation, not a file).

## F2 — server-only content is filtered by WHAT IT IS, not what the file is called {#f2}

_2026-08-05. The one real decision in this stream._

The edge withholds biome definitions from clients. Today: `n != "biomes.toml"` — a basename check.
Under [F1](#f1) a package can name its files anything, so `mods/foo/my-biomes.toml` would ship
worldgen rules to every client, silently, and nothing would ever say so.

**Chosen**: the edge filters on the **`[[biome]]` blocks themselves**, not on filenames. A served
file is the authored text with biome definitions removed; a file that is *only* biomes is not served
at all. The rule becomes "clients never receive biome definitions", which is a statement about data
and therefore cannot be defeated by naming.

It costs the edge a parse-and-reserialize on the served path where it previously passed raw text
through — acceptable because that path already re-reads and re-fingerprints the whole corpus on a
poll, and because the alternative is a rule that a mod author breaks by accident.

Rejected:

- **Keep the basename check.** A convention no one can enforce, whose failure is silent and
  client-side. The failure mode is "your mod leaked the worldgen rules and you find out never".
- **A reserved directory** (`content/server/**` never served). Cheap and explicit, but it splits the
  corpus by *deployment concern* rather than by content, so a package's biomes would have to live
  apart from the rest of that package — exactly the coupling [F1](#f1) exists to avoid.
- **A per-file `server_only = true` marker.** Authored metadata about a file, in the file — better
  than a name, but still a thing an author must remember, and biomes are the only case.

Note what is NOT claimed: this is a "don't ship what clients don't need" boundary, not a security
one. A determined client learns the worldgen rules by watching the world. The value is that the rule
now holds by construction rather than by everyone remembering a filename.

## F3 — the fingerprint hashes the RELATIVE PATH {#f3}

_2026-08-05, forced by [I1](issues.md#i1)._

`content_version` hashes each source's basename so that disk and R2 agree — they name the same file
differently and the fingerprint must not care. Nested, that is a collision:
`mods/a/things.toml` and `mods/b/things.toml` hash identically, and **moving a file between packages
changes the fingerprint not at all**, so the hot-reload poll never notices.

**Chosen**: hash the relative path. Disk and R2 already produce the same relative path — `load_disk`
strips the content root, `content_keys` strips the `<prefix>/content/` key prefix — so the property
the basename hack existed to preserve survives without the hack.

Rejected: **hashing the content only** (two files swapping names would go unnoticed);
**keeping basenames and forbidding duplicate basenames across packages** (a rule that makes
`mods/foo/things.toml` illegal, which is exactly the natural thing a package author would write).
