#!/usr/bin/env bash
# rd run — stand up the native tick-pipeline binaries (worker + master) for a
# local dev stack. Sourced by `bin/rd`; assumes common.sh is sourced and
# rd_resolve_env has run.
#
# Why this exists (issue 004): unlike edge/gateway (their own env containers),
# the worker and master connect **directly to SpacetimeDB via spacetimedb_sdk**,
# which links native-tls → needs libssl even though the local hop is plain http.
# So they can't reuse the npc rust:slim compose (npc talks ws:// to the edge, no
# TLS backend). Instead we keep ONE persistent `rd-run-<env>` container:
#   rust:slim + libssl-dev, repo bind-mounted at /w, on `--network host` so the
#   binaries reach the daemon at http://127.0.0.1:3000. cargo build lands in the
#   mounted target/, the binaries run detached (`docker exec -d`), logs → host.
# This is issue 004 option B, kept deliberately light (no compose/up-down wiring)
# — the imperative cousin of `rd deploy edge`.
#
#   rd run up        build + (re)run worker AND master (the self-driving stack)
#   rd run worker    build + (re)run just the worker
#   rd run master    build + (re)run just the master
#   rd run stop      kill worker + master (container stays for fast restarts)
#   rd run status    show what's running inside the container
#   rd run logs <worker|master> [-f]

WORKER_DIR="$REPO/server/worker"
MASTER_DIR="$REPO/server/master"

# The persistent build+run container for this env. Idempotent: create if absent,
# ensure libssl-dev once (marker file), then echo the container name.
rd_run_container() {
  local name="rd-run-${RD_ENV}"
  if ! docker inspect "$name" >/dev/null 2>&1; then
    rd_log "creating run container $name (rust:slim + libssl-dev, --network host)"
    docker run -d --name "$name" --network host -v "$REPO":/w -w /w \
      rust:slim sleep infinity >/dev/null
  elif [[ "$(docker inspect -f '{{.State.Running}}' "$name" 2>/dev/null)" != true ]]; then
    docker start "$name" >/dev/null
  fi
  if ! docker exec "$name" test -f /rd-deps-ok 2>/dev/null; then
    rd_log "installing libssl-dev in $name (one-time)"
    docker exec "$name" sh -c \
      'apt-get update -qq && apt-get install -y -qq libssl-dev pkg-config ca-certificates >/dev/null 2>&1 && touch /rd-deps-ok' \
      || rd_die "failed to install run deps in $name"
  fi
  echo "$name"
}

# Kill a detached process inside the run container by binary path (rust:slim has
# no pkill; scan /proc — same trick as rd_stop_process for edge).
rd_run_stop_one() {
  local name="$1" bin="$2"
  docker exec "$name" sh -c '
    bin="$1"; pids=""
    for p in /proc/[0-9]*/; do
      exe=$(tr "\0" "\n" < "$p/cmdline" 2>/dev/null | head -1)
      [ "$exe" = "$bin" ] && pids="$pids $(basename "$p")"
    done
    [ -n "$pids" ] || exit 0
    kill $pids 2>/dev/null || true; sleep 1
    for q in $pids; do kill -KILL "$q" 2>/dev/null || true; done
  ' sh "$bin" 2>/dev/null || true
}

# Build one binary (debug) inside the run container; deps cache in the mounted
# target/. $1 = crate dir under /w, $2 = human name.
rd_run_build() {
  local name; name="$(rd_run_container)"
  rd_log "build $2 → cargo build (in $name)"
  docker exec -w "/w/${1}" "$name" cargo build 2>&1 | tail -1
}

# (Re)run worker/master detached against this env's shard DB. The daemon is on
# the host (:3000 published), so ST_URI=http://127.0.0.1:3000 over --network host.
rd_run_worker() {
  local name; name="$(rd_run_container)"
  rd_run_build server/worker "worker"
  rd_run_stop_one "$name" /w/server/worker/target/debug/worker
  docker exec -d \
    -e ST_URI=http://127.0.0.1:3000 \
    -e SHARD_DB="$(rd_db_for zone)" \
    -e WORKER_ID="${WORKER_ID:-1}" \
    -e RUST_LOG="${RUST_LOG:-worker=info}" \
    "$name" sh -c 'exec /w/server/worker/target/debug/worker > /w/server/worker/worker-'"$RD_ENV"'.log 2>&1'
  rd_log "worker:$RD_ENV running (shard $(rd_db_for zone); log: server/worker/worker-$RD_ENV.log)"
}

rd_run_master() {
  local name; name="$(rd_run_container)"
  rd_run_build server/master "master"
  rd_run_stop_one "$name" /w/server/master/target/debug/master
  docker exec -d \
    -e ST_URI=http://127.0.0.1:3000 \
    -e SHARD_DB="$(rd_db_for zone)" \
    -e TIC_HZ="${TIC_HZ:-2}" \
    -e RUST_LOG="${RUST_LOG:-master=info}" \
    "$name" sh -c 'exec /w/server/master/target/debug/master > /w/server/master/master-'"$RD_ENV"'.log 2>&1'
  rd_log "master:$RD_ENV running (shard $(rd_db_for zone); log: server/master/master-$RD_ENV.log)"
}

rd_run() {
  local what="${1:-up}"; [[ -n "$what" ]] && shift || true
  case "$what" in
    up)     rd_run_master; rd_run_worker ;;
    worker) rd_run_worker ;;
    master) rd_run_master ;;
    stop)
      local name="rd-run-${RD_ENV}"
      docker inspect "$name" >/dev/null 2>&1 || { rd_warn "no run container for $RD_ENV"; return 0; }
      rd_run_stop_one "$name" /w/server/worker/target/debug/worker
      rd_run_stop_one "$name" /w/server/master/target/debug/master
      rd_log "stopped worker + master ($name kept for fast restart — 'docker rm -f $name' to drop)" ;;
    status)
      local name="rd-run-${RD_ENV}"
      docker inspect "$name" >/dev/null 2>&1 || { rd_warn "no run container for $RD_ENV"; return 0; }
      docker exec "$name" sh -c '
        for p in /proc/[0-9]*/; do
          exe=$(tr "\0" "\n" < "$p/cmdline" 2>/dev/null | head -1)
          case "$exe" in */worker|*/master) echo "running: $exe" ;; esac
        done' || true ;;
    logs)
      local which="${1:-worker}"
      rd_tail_log "$REPO/server/$which/$which-${RD_ENV}.log" "${2:-}" ;;
    -h|--help|help)
      cat >&2 <<EOF
usage: rd run <up|worker|master|stop|status|logs <worker|master> [-f]>

  up       build + (re)run worker AND master (self-driving tick pipeline)
  worker   build + (re)run just the worker
  master   build + (re)run just the master
  stop     kill worker + master (run container kept)
  status   show which pipeline binaries are running
  logs     tail worker/master log

Needs the daemon up (\`rd up spacetime\`) and the shard module published
(\`rd deploy module shard\`). Local-env only.
EOF
      ;;
    *) rd_warn "unknown run target '$what'"; rd_run help; exit 1 ;;
  esac
}
