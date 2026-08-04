# Completed — content is TOML-only

_Dated evidence: what landed and how it was checked. Append chronologically._

## 2026-08-04 · P0 — the inventory, frozen (1/1)

`git ls-files content` measured, not assumed: 12 tracked paths, 5 of them the corpus. Every one
of the other 7 traced to its writer and its readers by grep, recorded in
[`issues.md` I1](issues.md#i1) with a verdict per path.

Three things the count turned up that nobody was looking for:

- **[I2](issues.md#i2)** — `content/manifest.json` is not just misplaced, it is stale AND
  load-bearing: it names five `.rd` keys deleted by the previous stream's P6, so the edge's R2
  content source 404s on its first fetch. Unnoticed because every configured environment runs the
  Disk source.
- **[I4](issues.md#i4)** — `bin/lib/def_span.py` still greps the deleted `.rd` dialect;
  `--all` now resolves **zero** stems, so `art leaf-span` silently stops stamping the span that
  `tex_manifest` uses to size a stem's pow2 packing square.
- **[I5](issues.md#i5)** — a *fourth* private copy of the content walk survived the previous
  stream's cull, in the edge's `load_disk`, still speaking `.rd`.

Also established for the plan: `read_content_dir` is flat and TOML-only, so nothing under
`content/visual/` or `content/servers/` can reach a `Bundle` — the exiles are inert. That is why
P4's gate checks the tracked file set rather than trusting the loader to reject a stray.
