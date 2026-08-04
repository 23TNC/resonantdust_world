#!/usr/bin/env bash
# rd index — seed / inspect the index routing directory from the env's servers
# manifest (deploy/servers/<env>). Sourced by `bin/rd`; assumes common.sh is
# sourced and rd_resolve_env has run.
#
# The index DB (resonantdust-<env>-index-0) is the routing table the gateway and
# world server read: which world servers exist (servers), where data shards live
# (shards), and how regions map to shards (region_shards). Nothing self-registers
# yet, so this is how those rows get there — by calling the index module's
# reducers (set_server / set_shard / assign_region) for each manifest row.
#
#   rd index seed     write deploy/servers/<env> into the env's index DB
#   rd index show     dump the current servers / shards / region_shards rows
#
# Needs the spacetime daemon up (`rd up spacetime`) and the `index` module
# published (`rd deploy module index` or `rd redeploy --run`). `rd redeploy` calls
# the seed automatically after (re)publishing the index module or when the
# manifest changes.

# Normalize a manifest id token (hex `0x..` or decimal) to a decimal the spacetime
# CLI accepts. printf %d understands both forms.
rd_idx_num() { printf '%d' "$1" 2>/dev/null || rd_die "index manifest: '$1' is not a number"; }

# Seed the active env's index DB from its servers manifest. Idempotent: every
# reducer is an upsert, so re-running just refreshes the rows (and bumps the
# server heartbeat). Aborts on a malformed row rather than half-seeding silently.
rd_seed_index() {
  local manifest; manifest="$(rd_servers_manifest)"
  [[ -f "$manifest" ]] \
    || rd_die "no servers manifest at ${manifest/#$REPO\//} — create it to seed env '$RD_ENV'"

  local db now kind a b c d e
  db="$(rd_db_for index)"
  now="$(date +%s%3N)"   # heartbeat stamp (epoch ms), host clock
  rd_log "seed index $db ← ${manifest/#$REPO\//}"

  # Read the manifest on FD 3, not stdin: the reducer calls below run `docker compose exec` which
  # attaches (and drains) the loop body's stdin — on stdin that ate the rest of the manifest, so only
  # the first row ever seeded. FD 3 keeps the manifest out of the body's reach.
  local seeded=0
  while read -r kind a b c d e _ <&3; do
    [[ -z "$kind" || "$kind" == \#* ]] && continue
    case "$kind" in
      server)
        [[ -n "$a" && -n "$b" ]] || rd_die "index manifest: 'server' needs <id> <ws_url> (got: $kind $a $b)"
        rd_log "  set_server $a $b"
        rd_stc call --server local "$db" set_server "$(rd_idx_num "$a")" "$b" "$now" ;;
      shard)
        [[ -n "$a" && -n "$b" && -n "$c" ]] || rd_die "index manifest: 'shard' needs <id> <st_url> <db_name> (got: $kind $a $b $c)"
        rd_log "  set_shard $a $b $c"
        rd_stc call --server local "$db" set_shard "$(rd_idx_num "$a")" "$b" "$c" ;;
      region)
        [[ -n "$a" && -n "$b" ]] || rd_die "index manifest: 'region' needs <region_id> <shard_id> (got: $kind $a $b)"
        rd_log "  assign_region $a → shard $b"
        rd_stc call --server local "$db" assign_region "$(rd_idx_num "$a")" "$(rd_idx_num "$b")" ;;
      cold)
        [[ -n "$a" && -n "$b" && -n "$c" && -n "$d" && -n "$e" ]] \
          || rd_die "index manifest: 'cold' needs <type_id> <region> <shard> <st_url> <db_name> (got: $kind $a $b $c $d $e)"
        rd_log "  set_cold_shard type=$a region=$b shard=$c → $e"
        rd_stc call --server local "$db" set_cold_shard "$(rd_idx_num "$a")" "$(rd_idx_num "$b")" "$(rd_idx_num "$c")" "$d" "$e" ;;
      *)
        rd_die "index manifest: unknown row kind '$kind' (want: server | shard | region | cold)" ;;
    esac
    seeded=$((seeded + 1))
  done 3< "$manifest"

  rd_log "seeded $seeded row(s) into $db"
}

# Dump the index's routing rows for the active env — a quick "what does the
# gateway/server actually see right now" check, independent of the manifest.
rd_show_index() {
  local db; db="$(rd_db_for index)"
  rd_log "index $db (live rows)"
  local t
  for t in servers shards region_shards player_servers; do
    echo "── $t ──"
    rd_stc sql --server local "$db" "SELECT * FROM $t" || true
  done
}

rd_index() {
  local sub="${1:-}"; [[ -n "$sub" ]] && shift || { rd_index_usage; exit 1; }
  case "$sub" in
    seed)           rd_seed_index "$@" ;;
    show|ls|status) rd_show_index "$@" ;;
    -h|--help|help) rd_index_usage ;;
    *) rd_warn "unknown index subcommand '$sub'"; rd_index_usage; exit 1 ;;
  esac
}

rd_index_usage() {
  cat >&2 <<EOF
usage: rd index <seed|show>

  seed   write deploy/servers/$RD_ENV into the env's index DB
         (resonantdust-$RD_ENV-index-0) via the index module's reducers
  show   dump the current servers / shards / region_shards / player_servers rows

Needs \`rd up spacetime\` and a published \`index\` module. \`rd redeploy --run\`
seeds automatically after (re)publishing index or when the manifest changes.
EOF
}
