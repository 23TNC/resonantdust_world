#!/usr/bin/env bash
# rd build — compile a component. Sourced by `bin/rd` (and reused by redeploy).
# Components: shared, server, gateway, spacetime, client, pixijs. Everything Rust
# builds in docker (no host cargo); pixijs builds with the host's node/npm.
#
# Assumes common.sh is already sourced and rd_resolve_env has run.

# shared → the browser wasm bundle (shared/pkg/*) the pixijs client imports.
# Builds in docker via shared/compose.yml's `wasm` service.
rd_build_shared() {
  rd_log "build shared → wasm bundle (shared/pkg)"
  docker compose -f "$SHARED_DIR/compose.yml" run --rm wasm
}

# server → the release binary, in docker (rust:slim). One binary serves every
# env. `--build` keeps the build image in sync with the compose build service.
rd_build_server() {
  rd_log "build server → release binary (server/target/release/server)"
  rd_server_dcl run --rm --build build
}

# gateway → the release binary, in docker (rust:slim). One binary serves every
# env. `--bindings` first regenerates the SDK bindings (gateway/src/bindings/*)
# from the spacetime modules, then builds.
rd_build_gateway() {
  if [[ "${1:-}" == "--bindings" ]]; then
    rd_log "regenerate gateway bindings (gateway/src/bindings)"
    rd_gateway_dcl run --rm bindings
  fi
  rd_log "build gateway → release binary (gateway/target/release/gateway)"
  rd_gateway_dcl run --rm --build build
}

# spacetime → per-module wasm + server bindings. With no args, every module;
# otherwise the named ones. This is the "complicated" component: each module is
# its own crate under spacetime/server/modules/, built with the build service's
# workdir pointed at that crate, then its server bindings are regenerated.
rd_build_spacetime() {
  local mods=("$@")
  if [[ ${#mods[@]} -eq 0 ]]; then
    mapfile -t mods < <(rd_list_modules)
    [[ ${#mods[@]} -gt 0 ]] || rd_die "no modules found under $MODULES_DIR"
  fi
  local mod
  for mod in "${mods[@]}"; do
    rd_require_module "$mod"
    rd_log "build spacetime module: $mod"
    rd_st_dcl run --rm --workdir "/workspace/server/modules/$mod" build
    # `compose run` replaces the service command with trailing args, so re-supply
    # the bindings script name alongside the module arg.
    rd_log "generate server bindings: $mod"
    rd_st_dcl run --rm bindings generate-bindings.sh "$mod"
  done
}

# client → the headless client (library + `headless` binary), in docker
# (rust:slim). No TLS deps (every env is plain ws://), so stock rust:slim builds
# it. `--check` type-checks (lib + bin + tests) without producing a binary — the
# quick inner-loop check — instead of the full release build.
rd_build_client() {
  if [[ "${1:-}" == "--check" ]]; then
    rd_log "check client → cargo check --all-targets"
    rd_client_dcl run --rm check
    return
  fi
  rd_log "build client → release binary (client/target/release/headless)"
  rd_client_dcl run --rm build
}

# pixijs → the production web bundle (pixijs/dist). Uses the host npm; installs
# deps first if node_modules is missing. (This is the dumb display over the
# headless `client`; built on the host, not in docker.)
rd_build_pixijs() {
  rd_log "build pixijs → pixijs/dist"
  [[ -d "$PIXIJS_DIR/node_modules" ]] || ( cd "$PIXIJS_DIR" && npm install )
  ( cd "$PIXIJS_DIR" && npm run build )
}

rd_build_usage() {
  cat >&2 <<EOF
usage: rd build <component> [args]

components:
  shared              the browser wasm bundle (shared/pkg/*)
  server              the server release binary (in docker)
  gateway [--bindings]  the gateway release binary (in docker); --bindings first
                      regenerates gateway/src/bindings from the spacetime modules
  spacetime [mod...]  every module's wasm + server bindings, or only the named
                      modules (e.g. 'rd build spacetime shard chat')
  client [--check]    the headless client binary (in docker); --check type-checks
                      only (cargo check --all-targets)
  pixijs              the pixijs production web bundle (pixijs/dist)
EOF
}

rd_build() {
  local comp="${1:-}"
  [[ -n "$comp" ]] || { rd_build_usage; exit 1; }
  shift
  case "$comp" in
    shared)    rd_build_shared "$@" ;;
    server)    rd_build_server "$@" ;;
    gateway)   rd_build_gateway "$@" ;;
    spacetime) rd_build_spacetime "$@" ;;
    client)    rd_build_client "$@" ;;
    pixijs)    rd_build_pixijs "$@" ;;
    -h|--help|help) rd_build_usage ;;
    *) rd_warn "unknown component '$comp'"; rd_build_usage; exit 1 ;;
  esac
}
