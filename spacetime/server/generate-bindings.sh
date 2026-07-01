#!/bin/bash
# Generate Rust bindings for one SpacetimeDB module (consumed by the server).
#
# Usage: generate-bindings.sh <module-name>
#
# Reads the module from `/workspace/server/modules/<module>/` and emits:
#   - Rust bindings for the server, to
#     `<server>/src/bindings/<module>/`
#
# Driven by `rd build spacetime` — see the `bindings` compose service. NOTE: the
# spacetime modules workspace is mounted at /workspace/server inside the
# container, so the GAME server dir (host `../server`) is mounted at the distinct
# path /workspace/game-server to avoid the name clash. The output dir below is
# that container path.
set -e

SCRIPT_DIR="$(realpath "$(dirname "${BASH_SOURCE[0]}")")"

MODULE="${1:-}"
if [ -z "$MODULE" ]; then
  echo "usage: generate-bindings.sh <module>"
  echo "       e.g. generate-bindings.sh shard"
  exit 1
fi

MODULE_DIR="$SCRIPT_DIR/modules/$MODULE"
SERVER_BINDINGS_DIR="/workspace/game-server/src/bindings"
SERVER_OUT_DIR="$SERVER_BINDINGS_DIR/$MODULE"

if [ ! -d "$MODULE_DIR" ]; then
  echo "Missing module directory: $MODULE_DIR"
  exit 1
fi

# --- Rust bindings (server) ---
mkdir -p "$SERVER_OUT_DIR"
echo "Generating Rust bindings for module: $MODULE"
spacetime generate --yes --lang rust --out-dir "$SERVER_OUT_DIR" --module-path "$MODULE_DIR"

# Rust (unlike TS) needs each module declared to be reachable. Keep
# `bindings/mod.rs` in sync by ensuring this module's `pub mod` line exists.
MOD_RS="$SERVER_BINDINGS_DIR/mod.rs"
touch "$MOD_RS"
if ! grep -q "^pub mod $MODULE;" "$MOD_RS"; then
  echo "pub mod $MODULE;" >> "$MOD_RS"
fi

echo "Done."
echo "Rust bindings:       $SERVER_OUT_DIR"
