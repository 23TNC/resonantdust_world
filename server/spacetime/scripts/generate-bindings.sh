#!/bin/bash
# Generate Rust bindings for one SpacetimeDB module, for the native server to
# consume.
#
# Usage: generate-bindings.sh <module-name>
#
# Reads `/workspace/server/modules/<module>/` and emits bindings to
# `/workspace/game-server/src/bindings/<module>/`.
#
# Runs inside the `bindings` compose service (driven by `bin/st bindings`). Both
# paths above are CONTAINER paths: the module workspace already owns
# /workspace/server, so the edge is mounted at the distinct /workspace/game-server
# to avoid the clash. On the host those are `server/spacetime/modules/` and
# `server/edge/` respectively.
set -e

MODULES_DIR="/workspace/server/modules"
SERVER_BINDINGS_DIR="/workspace/game-server/src/bindings"

MODULE="${1:-}"
if [ -z "$MODULE" ]; then
  echo "usage: generate-bindings.sh <module>"
  echo "       e.g. generate-bindings.sh template"
  exit 1
fi

MODULE_DIR="$MODULES_DIR/$MODULE"
OUT_DIR="$SERVER_BINDINGS_DIR/$MODULE"

if [ ! -d "$MODULE_DIR" ]; then
  echo "Missing module directory: $MODULE_DIR"
  exit 1
fi

mkdir -p "$OUT_DIR"
echo "Generating Rust bindings for module: $MODULE"
spacetime generate --yes --lang rust --out-dir "$OUT_DIR" --module-path "$MODULE_DIR"

# Rust (unlike TS) needs each module declared to be reachable. Keep
# `bindings/mod.rs` in sync by ensuring this module's `pub mod` line exists.
MOD_RS="$SERVER_BINDINGS_DIR/mod.rs"
touch "$MOD_RS"
if ! grep -q "^pub mod $MODULE;" "$MOD_RS"; then
  echo "pub mod $MODULE;" >> "$MOD_RS"
fi

echo "Done. Rust bindings: $OUT_DIR"
