#!/usr/bin/env bash
# rd common library — sourced by `bin/rd` and the lib/* supporting scripts.
# Holds the env/profile resolution (the single source of truth for WHERE a
# command operates), repo paths, the docker-compose helpers, and small logging
# utilities. No top-level side effects beyond defining vars/functions, so it is
# safe to source from anywhere.

# ── paths ────────────────────────────────────────────────────────────────────
# This file lives at $REPO/bin/lib/common.sh, so the repo root is two up.
RD_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$RD_LIB_DIR/../.." && pwd)"
BIN="$REPO/bin"

SHARED_DIR="$REPO/shared"
EDGE_DIR="$REPO/server/edge"
GATEWAY_DIR="$REPO/server/gateway"
CORE_DIR="$REPO/client/core"
NPC_DIR="$REPO/client/npc"
SPACETIME_DIR="$REPO/server/spacetime"
MODULES_DIR="$SPACETIME_DIR/server/modules"
PIXIJS_DIR="$REPO/client/pixijs"
CONTENT_DIR="$REPO/content"

# Persisted active profile (written by `rd config set`), and per-env build state
# (redeploy stamps). Both live under bin/, which the repo .gitignore excludes —
# so they stay local, same as the old bin/.st-profile + spacetime/.build-state.
PROFILE_FILE="$BIN/.rd-profile"
STATE_ROOT="$BIN/.build-state"

# ── logging ──────────────────────────────────────────────────────────────────
rd_log()  { printf '[rd] %s\n' "$*" >&2; }
rd_warn() { printf '[rd] ⚠  %s\n' "$*" >&2; }
rd_die()  { printf '[rd] error: %s\n' "$*" >&2; exit 1; }

# ── env / profile resolution ─────────────────────────────────────────────────
# Profiles == environments. The active one fully determines the spacetime
# the spacetime server, the server port/service, the docker context, and the
# compose files.
#
# Precedence (highest first):
#   1. RD_PROFILE exported for this invocation (this is also how `rd --env <p>`
#      passes a one-shot override — the dispatcher validates then exports it);
#   2. the persisted default in bin/.rd-profile (via `rd config set`);
#   3. `dev`.
# Ambient RD_ENV / ST_* exports are deliberately NOT consulted — switch targets
# by switching profile, not by exporting vars (that shadowing silently sent the
# old tooling at the wrong env).
RD_KNOWN_PROFILES="dev claude test alpha"

rd_resolve_env() {
  if [[ -n "${RD_PROFILE:-}" ]]; then
    RD_PROFILE_SOURCE="RD_PROFILE env"
  else
    RD_PROFILE="$(cat "$PROFILE_FILE" 2>/dev/null || echo dev)"
    RD_PROFILE_SOURCE="profile file"
  fi

  case "$RD_PROFILE" in
    dev|claude|test)
      RD_ENV="$RD_PROFILE"
      RD_ST_SERVER="http://127.0.0.1:3000"
      RD_GATEWAY_HOST="localhost"
      RD_DOCKER_CONTEXT=""
      RD_REMOTE=0
      RD_ST_COMPOSE="$SPACETIME_DIR/compose.yml"
      RD_EDGE_COMPOSE="$EDGE_DIR/compose.yml"
      RD_GATEWAY_COMPOSE="$GATEWAY_DIR/compose.yml"
      ;;
    alpha)
      # Remote lightsail box: spacetime is plain HTTP on :3000, the server runs
      # there too (deploy.yml). Built locally, pushed in.
      RD_ENV="alpha"
      RD_ST_SERVER="http://gateway.resonantdust.com:3000"
      RD_GATEWAY_HOST="gateway.resonantdust.com"
      RD_DOCKER_CONTEXT="lightsail"
      RD_REMOTE=1
      RD_ST_COMPOSE="$SPACETIME_DIR/deploy.yml"
      RD_EDGE_COMPOSE="$EDGE_DIR/deploy.yml"
      # No gateway deploy.yml yet — remote gateway standup isn't wired (the
      # deploy primitives guard on this being empty). Build still works locally.
      RD_GATEWAY_COMPOSE=""
      ;;
    *)
      rd_die "unknown profile '$RD_PROFILE' (from $RD_PROFILE_SOURCE; known: $RD_KNOWN_PROFILES)"
      ;;
  esac

  # Per-env ports (the local three run side by side):
  #   • gateway (9473/4/5) — the CLIENT-FACING entry. Clients connect here to
  #     determine/acquire a server; they never hardcode a server address. Not
  #     implemented yet, so this is a reserved address (no compose service).
  #   • edge    (8473/4/5) — the game/edge server the tooling builds + stands up.
  #     The binary is env-agnostic; only the port/service/log differ. 8xxx=edge,
  #     9xxx=gateway.
  case "$RD_ENV" in
    dev)    RD_GATEWAY_PORT=9473; RD_GATEWAY_SERVICE=gateway;        RD_EDGE_PORT=8473; RD_EDGE_SERVICE=edge ;;
    claude) RD_GATEWAY_PORT=9474; RD_GATEWAY_SERVICE=gateway-claude; RD_EDGE_PORT=8474; RD_EDGE_SERVICE=edge-claude ;;
    test)   RD_GATEWAY_PORT=9475; RD_GATEWAY_SERVICE=gateway-test;   RD_EDGE_PORT=8475; RD_EDGE_SERVICE=edge-test ;;
    alpha)  RD_GATEWAY_PORT=9473; RD_GATEWAY_SERVICE=gateway-alpha;  RD_EDGE_PORT=8473; RD_EDGE_SERVICE=edge-alpha ;;
  esac
  # The address clients connect to (the gateway). A URL, per the old gate-url
  # contract the client baked in.
  RD_GATEWAY="http://${RD_GATEWAY_HOST}:${RD_GATEWAY_PORT}"

  # The SpacetimeDB daemon's address as seen *from inside* the resonantdust
  # docker network: it's the `start` service, regardless of env. Distinct from
  # RD_ST_SERVER (the host-facing URL the tooling/CLI use). Services we stand up
  # in containers (edge, gateway) reach the daemon here, not on 127.0.0.1.
  RD_STDB_INTERNAL="http://start:3000"

  # Per-module DBs are resonantdust-<env>-<module>-<idx>.
  RD_DB_PREFIX="resonantdust-${RD_ENV}"
  RD_STATE_DIR="$STATE_ROOT/$RD_ENV"
}

# DB name for a module + shard index (default 0): resonantdust-<env>-<mod>-<idx>.
rd_db_for() { echo "${RD_DB_PREFIX}-${1}-${2:-0}"; }

# The active env's index routing manifest (content/servers/<env>): the topology
# `rd index seed` writes into the env's index DB. One file per env.
rd_servers_manifest() { echo "$CONTENT_DIR/servers/$RD_ENV"; }

# ── docker helpers ───────────────────────────────────────────────────────────
# Ensure the external `resonantdust` network exists on the active context. All
# compose stacks join it (it's declared `external: true`), so it must be created
# out-of-band before the first `up`. Idempotent; honours the env's context so
# `alpha` creates it on the remote daemon.
rd_net_ensure() {
  docker ${RD_DOCKER_CONTEXT:+--context "$RD_DOCKER_CONTEXT"} \
    network inspect resonantdust >/dev/null 2>&1 && return 0
  rd_log "creating docker network: resonantdust${RD_DOCKER_CONTEXT:+ @ $RD_DOCKER_CONTEXT}"
  docker ${RD_DOCKER_CONTEXT:+--context "$RD_DOCKER_CONTEXT"} \
    network create resonantdust >/dev/null
}

# Spacetime compose, honouring the env's docker context + project dir.
rd_st_dc() {
  docker ${RD_DOCKER_CONTEXT:+--context "$RD_DOCKER_CONTEXT"} \
    compose -f "$RD_ST_COMPOSE" --project-directory "$SPACETIME_DIR" "$@"
}
# Spacetime build/codegen — ALWAYS the local daemon + compose.yml (deploy.yml
# has no build service; the remote box can't compile).
rd_st_dcl() {
  docker compose -f "$SPACETIME_DIR/compose.yml" --project-directory "$SPACETIME_DIR" "$@"
}
# Run the spacetime CLI inside the running `start` container (the one
# identity-stable path — the container's server-issued identity never drifts).
rd_stc() { rd_st_dc exec -T start spacetime "$@"; }

# Edge compose, honouring the env's docker context.
rd_edge_dc() {
  docker ${RD_DOCKER_CONTEXT:+--context "$RD_DOCKER_CONTEXT"} \
    compose -f "$RD_EDGE_COMPOSE" "$@"
}
# Edge build — always local compose.yml (one binary serves every env).
rd_edge_dcl() {
  docker compose -f "$EDGE_DIR/compose.yml" "$@"
}
# Gateway compose, honouring the env's docker context. Used to stand up + run the
# gateway binary (mirrors rd_edge_dc). Guards on RD_GATEWAY_COMPOSE being set —
# it's empty for envs with no gateway standup wired (e.g. alpha, no deploy.yml).
rd_gateway_dc() {
  [[ -n "$RD_GATEWAY_COMPOSE" ]] || rd_die "no gateway standup for env '$RD_ENV' (RD_GATEWAY_COMPOSE unset)"
  docker ${RD_DOCKER_CONTEXT:+--context "$RD_DOCKER_CONTEXT"} \
    compose -f "$RD_GATEWAY_COMPOSE" "$@"
}
# Gateway build — always local compose.yml (one binary serves every env).
rd_gateway_dcl() {
  docker compose -f "$GATEWAY_DIR/compose.yml" "$@"
}
# Headless client-core build — always local compose.yml (a dev-side library + bin,
# not an env-specific deployment).
rd_core_dcl() {
  docker compose -f "$CORE_DIR/compose.yml" "$@"
}
# NPC driver build/run — a dev-side bot binary, always the local compose.yml.
rd_npc_dcl() {
  docker compose -f "$NPC_DIR/compose.yml" "$@"
}

# ── module discovery ─────────────────────────────────────────────────────────
# Each spacetime/server/modules/<dir> with a Cargo.toml is a module.
rd_list_modules() {
  [[ -d "$MODULES_DIR" ]] || return 0
  local d
  for d in "$MODULES_DIR"/*/; do
    [[ -f "${d}Cargo.toml" ]] || continue
    basename "${d%/}"
  done
}

rd_require_module() {
  [[ -f "$MODULES_DIR/$1/Cargo.toml" ]] || {
    rd_warn "unknown module: $1"
    rd_log "known modules: $(rd_list_modules | tr '\n' ' ')"
    exit 1
  }
}
