#!/usr/bin/env bash
# rd stack — container lifecycle + standalone deploy. Sourced by `bin/rd`;
# assumes common.sh (and, for `deploy`, build.sh + redeploy.sh) are sourced and
# rd_resolve_env has run.
#
# Three stacks make up a running env, each its own compose project:
#   • spacetime — the SpacetimeDB daemon (`start`). SHARED across the local envs
#     (one daemon on :3000; envs differ only by DB name), so `up`/`down` act on
#     the single instance.
#   • server    — the world-server binary, run detached in the env's long-lived
#     container (server / server-claude / server-test), on 8473/4/5.
#   • gateway   — the directory binary, run detached in the env's long-lived
#     container (gateway / gateway-claude / gateway-test), on 9473/4/5.
#
# The build+run of the server/gateway *binaries* is `rd deploy <…>` (or, change-
# detected, `rd redeploy --run`); `rd up`/`down` only manage the containers (and
# the daemon). Targets: spacetime | server | gateway | all (default all).

# Resolve a target word to the work it touches. `all` = every stack for this env.
rd_stack_targets() {
  case "${1:-all}" in
    spacetime|st) echo spacetime ;;
    server)       echo server ;;
    gateway)      echo gateway ;;
    all)          echo "spacetime server gateway" ;;
    *) rd_die "unknown target '$1' (want: spacetime | server | gateway | all)" ;;
  esac
}

# ── rd up ────────────────────────────────────────────────────────────────────
# Stand up the requested stack's container(s). Always ensures the shared network
# first. This brings containers up only — for server/gateway it starts the idle
# (`sleep infinity`) container; run the binary inside it with `rd deploy <…>`.
rd_up() {
  local t; rd_net_ensure
  for t in $(rd_stack_targets "${1:-all}"); do
    case "$t" in
      spacetime) rd_log "up: spacetime daemon ($RD_ST_SERVER)"; rd_st_dc up -d start >/dev/null ;;
      server)    rd_log "up: server container ($RD_SERVER_SERVICE)"; rd_server_dc up -d "$RD_SERVER_SERVICE" >/dev/null ;;
      gateway)   rd_log "up: gateway container ($RD_GATEWAY_SERVICE)"; rd_gateway_dc up -d "$RD_GATEWAY_SERVICE" >/dev/null ;;
    esac
  done
}

# ── rd down ──────────────────────────────────────────────────────────────────
# Stop the requested stack. server/gateway `stop` only the active env's container
# (the other envs run side by side, so we never knock them over); spacetime stops
# the shared daemon. Containers are stopped, not removed — `rd up` restarts them.
rd_down() {
  local t
  for t in $(rd_stack_targets "${1:-all}"); do
    case "$t" in
      spacetime) rd_log "down: spacetime daemon"; rd_st_dc stop start >/dev/null 2>&1 || true ;;
      server)    rd_log "down: server container ($RD_SERVER_SERVICE)"; rd_server_dc stop "$RD_SERVER_SERVICE" >/dev/null 2>&1 || true ;;
      gateway)   rd_log "down: gateway container ($RD_GATEWAY_SERVICE)"; rd_gateway_dc stop "$RD_GATEWAY_SERVICE" >/dev/null 2>&1 || true ;;
    esac
  done
}

# ── rd ps ────────────────────────────────────────────────────────────────────
# Status across all three stacks for this env. The daemon is a compose service;
# server/gateway containers idle while their binary (if running) is a process
# inside — so this shows the containers, and the per-binary logs live in the
# files `rd logs` tails.
rd_ps() {
  echo "── spacetime (shared daemon) ──"; rd_st_dc ps || true
  echo "── server ($RD_ENV) ──";          rd_server_dc ps || true
  if [[ -n "$RD_GATEWAY_COMPOSE" ]]; then
    echo "── gateway ($RD_ENV) ──";       rd_gateway_dc ps || true
  fi
}

# ── rd logs ──────────────────────────────────────────────────────────────────
# spacetime → the daemon's compose logs. server/gateway → the detached binary's
# host log file (server/server-<env>.log, gateway/gateway-<env>.log), since the
# binary writes there, not to container stdout. Extra args (e.g. -f) pass through.
rd_logs() {
  local target="${1:-}"; [[ -n "$target" ]] && shift || rd_die "usage: rd logs <spacetime|server|gateway> [-f]"
  case "$target" in
    spacetime|st) rd_st_dc logs "$@" start ;;
    server)  rd_tail_log "$SERVER_DIR/server-${RD_ENV}.log" "$@" ;;
    gateway) rd_tail_log "$GATEWAY_DIR/gateway-${RD_ENV}.log" "$@" ;;
    *) rd_die "unknown target '$target' (want: spacetime | server | gateway)" ;;
  esac
}
# Tail a host log file; `-f` follows. Missing file is reported, not fatal (the
# binary may simply not have been deployed yet).
rd_tail_log() {
  local f="$1"; shift
  [[ -f "$f" ]] || { rd_warn "no log yet at ${f/#$REPO\//} (deploy it first)"; return 0; }
  case " $* " in *" -f "*) tail -f "$f" ;; *) tail -n "${1:-200}" "$f" ;; esac
}

# ── rd deploy ────────────────────────────────────────────────────────────────
# Build + run one component now, no change detection (the imperative cousin of
# `rd redeploy`). Needs the relevant container(s) up — `rd up <target>` first, or
# these `up` it themselves via the deploy primitives.
#
#   rd deploy server                 build the server binary, (re)run it detached
#   rd deploy gateway                build the gateway binary, (re)run it detached
#   rd deploy module <name> [--keep] publish a module's wasm (--keep = no data wipe)
rd_deploy() {
  local what="${1:-}"; [[ -n "$what" ]] && shift || { rd_deploy_usage; exit 1; }
  case "$what" in
    server)
      [[ "$RD_REMOTE" == 0 ]] || rd_die "server is remote for '$RD_ENV' — deploy it separately"
      rd_build_server; rd_deploy_server ;;
    gateway)
      [[ "$RD_REMOTE" == 0 ]] || rd_die "no remote gateway standup wired for '$RD_ENV'"
      rd_build_gateway; rd_deploy_gateway ;;
    module)
      local mod="${1:-}"; [[ -n "$mod" ]] || rd_die "usage: rd deploy module <name> [--keep]"
      shift
      local reset=1; [[ "${1:-}" == "--keep" ]] && reset=0
      rd_deploy_module "$mod" "$reset" ;;
    -h|--help|help) rd_deploy_usage ;;
    *) rd_warn "unknown deploy target '$what'"; rd_deploy_usage; exit 1 ;;
  esac
}

rd_deploy_usage() {
  cat >&2 <<EOF
usage: rd deploy <server|gateway|module <name>>

  server                 build + (re)run the world-server binary in its container
  gateway                build + (re)run the gateway binary in its container
  module <name> [--keep] publish a spacetime module's wasm (--keep keeps data)

Needs the spacetime daemon up (\`rd up spacetime\`) for module publishes, and is
local-env only for server/gateway. For change-detected bulk deploys use
\`rd redeploy --run\`.
EOF
}
