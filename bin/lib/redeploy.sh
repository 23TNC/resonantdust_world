#!/usr/bin/env bash
# rd redeploy — detect which build units changed since the last redeploy and
# rebuild + redeploy only those. Sourced by `bin/rd`; assumes common.sh and
# build.sh are sourced and rd_resolve_env has run.
#
# Detection is content-hash based, not git-diff: it catches uncommitted edits,
# means "since last build" (not "since last commit"), and is idempotent. Stamps
# live per-env under bin/.build-state/<env> (gitignored); a unit is stamped
# clean only after its action succeeds, so a failed build stays dirty and
# retries next run.
#
#   rd redeploy              # dry run — print the plan, touch nothing
#   rd redeploy --run        # execute the plan
#   rd redeploy --run --no-reset   # republish without wiping data
#   rd redeploy --force      # treat every unit as changed
#   rd redeploy --mark       # stamp all units clean WITHOUT building (baseline)
#
# Target env follows the active profile (`rd config set <p>` / RD_PROFILE).

# Units split into two kinds:
#   • spacetime modules — discovered dynamically from spacetime/server/modules/
#     (so a module rename/add/remove needs no edit here). Each module's closure
#     is its own dir; its deploy DB target defaults to its name.
#   • the rest (edge, gateway, shared, webgl) — explicit, hand-written
#     closures. edge + gateway are local-only standups (build the binary, run
#     it detached in the env's long-lived container); both are skipped on remote
#     envs, which have no in-repo standup wired.
# A unit is "changed" when the hash over its inputs differs from its last clean
# stamp. Add a shared-crate path to a closure once a unit actually links it (e.g.
# append "$SHARED_DIR" to edge when the edge links resonantdust-shared) so
# editing it marks the dependent dirty — kept honest: nothing path-deps shared yet.
#
# The headless client-core crate is NOT a redeploy unit: it produces a dev-side
# binary nothing deployed consumes yet, so it has no deploy step. Build it on
# demand with `rd build core`. Fold it in here once webgl links it.
declare -A RD_INPUTS=()
RD_MODULE_UNITS=()
rd_init_units() {
  local mod
  for mod in $(rd_list_modules); do
    RD_MODULE_UNITS+=("$mod")
    # shared/codec is compiled INTO every module (path dep) — without it in the hash a
    # codec change (new action, layout) deploys NOTHING and the live validators reject
    # the new wire (movement-hardening: MOVE_STEP was silently unpublishable).
    RD_INPUTS[$mod]="$MODULES_DIR/$mod $SHARED_DIR/codec"
  done
  RD_INPUTS[edge]="$EDGE_DIR/src $EDGE_DIR/Cargo.toml $SHARED_DIR/codec $SHARED_DIR/content"
  RD_INPUTS[gateway]="$GATEWAY_DIR/src $GATEWAY_DIR/Cargo.toml $GATEWAY_DIR/../uplink/src"
  RD_INPUTS[shared]="$SHARED_DIR"
  RD_INPUTS[webgl]="$WEBGL_DIR/src $WEBGL_DIR/index.html $WEBGL_DIR/package.json $WEBGL_DIR/vite.config.ts $WEBGL_DIR/tsconfig.json"
  # The index routing seed — not a build, but tracked so editing the env's
  # servers manifest re-seeds the index DB. Its action also runs whenever the
  # `index` module is (re)published below, since a --reset publish wipes the rows.
  RD_INPUTS[index-seed]="$(rd_servers_manifest)"
  # Stable iteration order (modules first, then the seed, then the rest).
  RD_ORDER=("${RD_MODULE_UNITS[@]}" index-seed edge gateway shared webgl)
}

# A module's deploy DB target(s) default to its own name. A module that backs
# more than one DB family overrides it, e.g.: RD_RE_TARGETS[zone_shard]="cards regions".
declare -A RD_RE_TARGETS=()
rd_re_targets() { echo "${RD_RE_TARGETS[$1]:-$1}"; }

# Binary paths inside the long-lived containers (each bind-mounts its crate at
# /workspace, so the release binary lands here).
RD_EDGE_BIN="/workspace/target/release/edge"
RD_GATEWAY_BIN="/workspace/target/release/gateway"

# sha256 over every SOURCE file under the given paths, order-independent.
# Excludes derived outputs (target/, target-wasm/, pkg/, dist/, node_modules/)
# and logs, all of which live inside a unit's own closure — without excluding
# them a build would rewrite its own inputs and never stamp clean. Missing paths
# hash to empty, so a deleted source reads as changed.
rd_hash_unit() {
  find "$@" -type f \
       -not -path '*/target/*' -not -path '*/target-wasm/*' \
       -not -path '*/pkg/*' -not -path '*/dist/*' -not -path '*/node_modules/*' \
       -not -name '*.log' -print0 2>/dev/null \
    | LC_ALL=C sort -z \
    | xargs -0 sha256sum 2>/dev/null \
    | sha256sum | cut -d' ' -f1
}

# ── deploy primitives ────────────────────────────────────────────────────────
# Publish one module's wasm to its DB. Builds the wasm locally, copies it into
# the running `start` container, and publishes from there with the container's
# persistent identity (--server local) — no source/toolchain on the box, and
# ownership stays stable across deploys. reset=1 wipes the DB; reset=0 keeps data.
rd_deploy_module() {
  local target="$1" reset="$2" idx=0
  local src; src="$target"   # module directory under spacetime/server/modules
  rd_require_module "$src"
  # DB family this module deploys to. Usually the module's own name — but a SpacetimeDB
  # database name may not contain underscores ("invalid characters in database name"), so the
  # rebuild's `event_shard` / `data_shard` modules map to hyphenated families. The module
  # *directory* + crate keep the underscore; only the DB name is hyphenated.
  local fam
  case "$target" in
    event_shard) fam="event-shard" ;;
    data_shard)  fam="data-shard" ;;
    player_pawn) fam="player-pawn" ;;
    *)           fam="$target" ;;
  esac
  rd_log "deploy module $src → $(rd_db_for "$fam" "$idx")"
  rd_st_dcl run --rm --workdir "/workspace/server/modules/$src" build
  local crate wasm
  crate="$(grep -m1 '^name' "$MODULES_DIR/$src/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
  wasm="$MODULES_DIR/$src/target/wasm32-unknown-unknown/release/${crate//-/_}.opt.wasm"
  [[ -f "$wasm" ]] || rd_die "no wasm produced at $wasm"
  local db incontainer; db="$(rd_db_for "$fam" "$idx")"; incontainer="/tmp/$(basename "$wasm")"
  rd_st_dc cp "$wasm" "start:$incontainer"
  local delete=()
  [[ "$reset" == 1 ]] && delete=(--delete-data=always)
  rd_stc publish --yes --server local --bin-path "$incontainer" "${delete[@]}" "$db"
}

# Stop a running detached *process* inside a container (the binary we exec'd, not
# PID 1's `sleep infinity`). rust:slim has no pkill/ps, so scan /proc and match
# the binary path. $1 = compose-runner fn (rd_edge_dc|rd_gateway_dc), $2 =
# service, $3 = binary path. Used before re-running so we never stack two copies.
rd_stop_process() {
  local dc="$1" svc="$2" bin="$3"
  "$dc" exec -T "$svc" sh -c '
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

# Run the (freshly built) edge binary detached inside its env container, on the
# env's port. SERVER_ENV pins the control-plane DB names to this env (without it
# the binary defaults to dev — wrong for claude/test). SERVER_STDB_URI is left at
# the binary's default (http://start:3000, the daemon on the shared network).
# (SERVER_* are the edge binary's runtime env-var contract, unchanged by the
# server→edge rename.) Output → server/edge/edge-<env>.log via the bind mount.
rd_deploy_edge() {
  local log="edge-${RD_ENV}.log"
  rd_edge_dc up -d "$RD_EDGE_SERVICE" >/dev/null
  rd_stop_process rd_edge_dc "$RD_EDGE_SERVICE" "$RD_EDGE_BIN"
  rd_edge_dc exec -d \
    -e RUST_LOG="${RUST_LOG:-info}" \
    -e SERVER_ENV="$RD_ENV" \
    -e SERVER_LISTEN="0.0.0.0:$RD_EDGE_PORT" \
    "$RD_EDGE_SERVICE" \
    sh -c "exec $RD_EDGE_BIN > /workspace/$log 2>&1"
  rd_log "edge:$RD_ENV running detached on :$RD_EDGE_PORT (log: server/edge/$log)"
}

# Run the (freshly built) gateway binary detached inside its env container, on the
# env's gateway port. The gateway is a thin directory over the `index` DB: it
# reads INDEX_DB (this env's resonantdust-<env>-index-0) on INDEX_URI (the daemon
# on the shared network). Output → gateway/gateway-<env>.log via the bind mount.
rd_deploy_gateway() {
  local log="gateway-${RD_ENV}.log"
  rd_gateway_dc up -d "$RD_GATEWAY_SERVICE" >/dev/null
  rd_stop_process rd_gateway_dc "$RD_GATEWAY_SERVICE" "$RD_GATEWAY_BIN"
  rd_gateway_dc exec -d \
    -e RUST_LOG="${RUST_LOG:-info}" \
    -e GATE_LISTEN="0.0.0.0:$RD_GATEWAY_PORT" \
    -e INDEX_URI="$RD_STDB_INTERNAL" \
    -e INDEX_DB="$(rd_db_for index)" \
    "$RD_GATEWAY_SERVICE" \
    sh -c "exec $RD_GATEWAY_BIN > /workspace/$log 2>&1"
  rd_log "gateway:$RD_ENV running detached on :$RD_GATEWAY_PORT (log: gateway/$log)"
}

# ── main ─────────────────────────────────────────────────────────────────────
rd_redeploy() {
  rd_init_units
  local RUN=0 RESET=1 FORCE=0 MARK=0
  for a in "$@"; do
    case "$a" in
      --run)      RUN=1 ;;
      --no-reset) RESET=0 ;;
      --force)    FORCE=1 ;;
      --mark)     MARK=1 ;;
      -h|--help)  sed -n '15,24p' "$RD_LIB_DIR/redeploy.sh" | sed 's/^# \{0,1\}//'; return 0 ;;
      *) rd_die "redeploy: unknown arg '$a'" ;;
    esac
  done

  mkdir -p "$RD_STATE_DIR"
  local -A NEWHASH CHANGED
  local CHANGED_UNITS=() u old
  for u in "${RD_ORDER[@]}"; do
    NEWHASH[$u]="$(rd_hash_unit ${RD_INPUTS[$u]})"
    old=""; [[ -f "$RD_STATE_DIR/$u" ]] && old="$(cat "$RD_STATE_DIR/$u")"
    if [[ "$FORCE" == 1 || "${NEWHASH[$u]}" != "$old" ]]; then CHANGED[$u]=1; CHANGED_UNITS+=("$u"); fi
  done

  # --mark: adopt the current tree as the clean baseline, build nothing.
  if [[ "$MARK" == 1 ]]; then
    for u in "${RD_ORDER[@]}"; do echo "${NEWHASH[$u]}" > "$RD_STATE_DIR/$u"; done
    rd_log "marked ${#RD_ORDER[@]} units clean at current state."
    return 0
  fi

  echo "env: $RD_ENV (profile $RD_PROFILE)    state: ${RD_STATE_DIR/#$REPO\//}"
  if [[ ${#CHANGED_UNITS[@]} -eq 0 ]]; then echo "nothing changed — up to date."; return 0; fi
  echo "changed: ${CHANGED_UNITS[*]}"

  # Plan.
  local EDGE_LOCAL=0; [[ "$RD_REMOTE" == 0 ]] && EDGE_LOCAL=1
  echo "--- plan ---"
  for u in "${RD_MODULE_UNITS[@]}"; do
    [[ -n "${CHANGED[$u]:-}" ]] || continue
    for t in $(rd_re_targets "$u"); do
      if [[ "$RESET" == 1 ]]; then printf '  %-26s # publish, wipe data\n' "deploy module $t"
      else                          printf '  %-26s # publish, keep data\n' "deploy module $t"; fi
    done
  done
  # The index seed runs when the manifest changed, OR when the index module was
  # (re)published above — a --reset publish wipes its rows, so they need rewriting.
  local SEED_INDEX=0
  [[ -n "${CHANGED[index-seed]:-}" || -n "${CHANGED[index]:-}" ]] && SEED_INDEX=1
  [[ "$SEED_INDEX" == 1 ]] && printf '  %-26s # %s\n' "seed index" "← ${RD_INPUTS[index-seed]/#$REPO\//}"
  if [[ -n "${CHANGED[edge]:-}" ]]; then
    if [[ "$EDGE_LOCAL" == 1 ]]; then printf '  %-26s # rebuild binary + restart\n' "build+deploy edge"
    else                                printf '  %-26s # skipped — edge is remote\n' "(edge)"; fi
  fi
  if [[ -n "${CHANGED[gateway]:-}" ]]; then
    if [[ "$EDGE_LOCAL" == 1 ]]; then printf '  %-26s # rebuild binary + restart\n' "build+deploy gateway"
    else                                printf '  %-26s # skipped — no remote gateway standup\n' "(gateway)"; fi
  fi
  [[ -n "${CHANGED[shared]:-}" ]] && printf '  %-26s # rebuild wasm bundle\n' "build shared"
  [[ -n "${CHANGED[webgl]:-}" ]] && printf '  %-26s # rebuild webgl bundle\n' "build webgl"

  if [[ "$RUN" == 0 ]]; then echo "--- dry run (pass --run to execute) ---"; return 0; fi

  # Execute. Stamp each unit only after its action succeeds.
  echo "--- running ---"
  # Stamp with a hash RECOMPUTED after the action: a deploy can generate files inside its own
  # inputs (a module build writes Cargo.lock), and stamping the pre-action hash made the very
  # next redeploy see a change — which for a module is a DATA-WIPING republish (first-pawns I1,
  # self-heal P5). target/ etc. are excluded by rd_hash_unit; Cargo.lock is a legit input.
  stamp() { rd_hash_unit ${RD_INPUTS[$1]} > "$RD_STATE_DIR/$1"; }
  local deployed_modules=0
  for u in "${RD_MODULE_UNITS[@]}"; do
    [[ -n "${CHANGED[$u]:-}" ]] || continue
    for t in $(rd_re_targets "$u"); do rd_deploy_module "$t" "$RESET"; done
    stamp "$u"
    deployed_modules=1
  done
  # A module publish wipes its DB and drops every connected SDK session. The sim processes +
  # npc SELF-HEAL (server/uplink + the client engine reconnect — sim-self-heal P2-P4), so no
  # bounce is needed; say who noticed, so a dev knows the blip is expected.
  if [[ "$deployed_modules" == 1 ]]; then
    local healers
    healers="$(docker ps --filter name=rd- --format '{{.Names}}' 2>/dev/null | tr '\n' ' ')"
    [[ -n "$healers" ]] && rd_log "module republished — live sim processes self-heal in place: $healers"
  fi
  # Re-seed the index routing directory once the (possibly just-republished) index
  # module is in place. Skips cleanly when this env has no manifest yet.
  if [[ "$SEED_INDEX" == 1 ]]; then
    if [[ -f "$(rd_servers_manifest)" ]]; then rd_seed_index; else
      rd_warn "no servers manifest for '$RD_ENV' (${RD_INPUTS[index-seed]/#$REPO\//}) — skipping index seed"
    fi
    stamp index-seed
  fi
  if [[ -n "${CHANGED[edge]:-}" ]]; then
    if [[ "$EDGE_LOCAL" == 1 ]]; then
      rd_build_edge; rd_deploy_edge; stamp edge
      rd_log "edge restarted — the npc auto-reconnects (engine heal); BROWSER tabs need a reload"
    else
      rd_log "note: '$RD_ENV' edge is remote — deploy it separately."; stamp edge
    fi
  fi
  if [[ -n "${CHANGED[gateway]:-}" ]]; then
    if [[ "$EDGE_LOCAL" == 1 ]]; then
      rd_build_gateway; rd_deploy_gateway; stamp gateway
    else
      rd_log "note: no remote gateway standup wired for '$RD_ENV' — skipping."; stamp gateway
    fi
  fi
  if [[ -n "${CHANGED[shared]:-}" ]]; then rd_build_shared; stamp shared; fi
  if [[ -n "${CHANGED[webgl]:-}" ]]; then rd_build_webgl; stamp webgl; fi
  echo "done."
}
