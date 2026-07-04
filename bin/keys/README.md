# bin/keys — local secrets

Runtime credential files, **gitignored**. The `.gitignore` here ignores
everything (`*`) except itself and this README, so nothing secret is ever
tracked. Create the files below by hand on each machine that needs them.

Each file is `KEY=value` lines (shell-style, `#` comments allowed). Tools read
them directly; you can also just export the same variables in your environment,
which takes precedence.

## `r2.env` — Cloudflare R2 object storage
Used by `art upload` (S3-compatible API). Only the two secrets live here;
account id / endpoint / bucket / prefix are non-secret and hard-coded in `bin/art`.

```
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
```

## `anthropic.env` — Anthropic API
Used by `art generate` to expand terse `--positive` / `--negative` prompts into
robust ones via Claude (Haiku). **Optional** — without it, `art generate` uses
the prompts verbatim (it prints a note and continues).

```
ANTHROPIC_API_KEY=sk-ant-...
```

## Notes
- Precedence: an exported env var (e.g. `ANTHROPIC_API_KEY`) overrides the file.
- Keep these files `chmod 600`. They must never be committed — the `.gitignore`
  enforces it, but double-check before `git add -A`.
