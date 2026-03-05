#!/usr/bin/env bash
set -euo pipefail

# Automated resume-soak orchestrator:
# 1) starts download_soak_stack_bg.sh
# 2) waits for a target scenario (default: concurrency) to be running
# 3) captures pre-crash download snapshot
# 4) hard-kills rust-mule (SIGKILL)
# 5) restarts rust-mule in the same staged run dir
# 6) verifies post-restart scenario progress
# 7) waits for stack completion and collects final tarball
#
# Usage:
#   scripts/test/download_resume_soak.sh
#   RESUME_SCENARIO=concurrency scripts/test/download_resume_soak.sh run
#
# Optional overrides:
#   STACK_SCRIPT=scripts/test/download_soak_stack_bg.sh
#   STACK_ROOT=/tmp/rust-mule-download-stack
#   STACK_API_PORT=17865
#   STACK_BASE_URL=http://127.0.0.1:17865
#   RESUME_SCENARIO=concurrency
#   WAIT_TIMEOUT_SECS=21600
#   HEALTH_TIMEOUT_SECS=300
#   ACTIVE_TRANSFER_TIMEOUT_SECS=1800
#   COMPLETION_TIMEOUT_SECS=3600
#   POLL_SECS=2
#   RESUME_OUT_DIR=/tmp/rust-mule-download-resume-<timestamp>
# Forwarded to stack/band runners:
#   DOWNLOAD_FIXTURES_FILE=/tmp/download_fixtures.json
#   FIXTURES_ONLY=1
#   FIXTURE_SOURCE_TIMEOUT_SECS=300
#   STACK_PUBLISH_FIXTURES=1
#   STACK_PUBLISH_BASE_URL=http://127.0.0.1:17865
#   STACK_PUBLISH_TOKEN_FILE=/path/to/publisher/api.token

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STACK_SCRIPT="${STACK_SCRIPT:-$SCRIPT_DIR/download_soak_stack_bg.sh}"
STACK_ROOT="${STACK_ROOT:-/tmp/rust-mule-download-stack}"
CONTROL_DIR="$STACK_ROOT/control"

STACK_API_PORT="${STACK_API_PORT:-17865}"
STACK_BASE_URL="${STACK_BASE_URL:-http://127.0.0.1:${STACK_API_PORT}}"
BASE_URL="$STACK_BASE_URL"
RESUME_SCENARIO="${RESUME_SCENARIO:-concurrency}"

WAIT_TIMEOUT_SECS="${WAIT_TIMEOUT_SECS:-21600}"
HEALTH_TIMEOUT_SECS="${HEALTH_TIMEOUT_SECS:-300}"
ACTIVE_TRANSFER_TIMEOUT_SECS="${ACTIVE_TRANSFER_TIMEOUT_SECS:-1800}"
NO_RESERVE_ACTIVITY_TIMEOUT_SECS="${NO_RESERVE_ACTIVITY_TIMEOUT_SECS:-300}"
COMPLETION_TIMEOUT_SECS="${COMPLETION_TIMEOUT_SECS:-3600}"
FIXTURE_SOURCE_TIMEOUT_SECS="${FIXTURE_SOURCE_TIMEOUT_SECS:-300}"
STACK_PUBLISH_FIXTURES="${STACK_PUBLISH_FIXTURES:-1}"
STACK_PUBLISH_BASE_URL="${STACK_PUBLISH_BASE_URL:-$BASE_URL}"
STACK_PUBLISH_TOKEN_FILE="${STACK_PUBLISH_TOKEN_FILE:-}"
POLL_SECS="${POLL_SECS:-2}"
PROGRESS_LOG_SECS="${PROGRESS_LOG_SECS:-30}"
RESUME_OUT_DIR="${RESUME_OUT_DIR:-/tmp/rust-mule-download-resume-$(date +%Y%m%d_%H%M%S)}"

RUN_DIR=""
STATUS_TSV=""
STACK_TARBALL=""
CRASH_EPOCH=0
RESTART_EPOCH=0
STACK_STARTED=0
STOP_ON_EXIT=1

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
ts_epoch() { date +%s; }
log() { echo "$(ts) $*"; }

cleanup_on_exit() {
  local rc="$?"
  if (( STOP_ON_EXIT == 0 )); then
    return
  fi
  if (( STACK_STARTED == 1 )); then
    "$STACK_SCRIPT" stop >/dev/null 2>&1 || true
    log "cleanup: stop requested for stack runner (exit_rc=$rc)"
  fi
  STOP_ON_EXIT=0
  trap - EXIT INT TERM
}

is_pid_alive() {
  local pid="$1"
  [[ -n "$pid" ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

require_tools() {
  command -v curl >/dev/null 2>&1 || {
    echo "ERROR: curl is required" >&2
    exit 1
  }
  command -v jq >/dev/null 2>&1 || {
    echo "ERROR: jq is required" >&2
    exit 1
  }
}

valid_fixture_filter='
  .[]? |
  select(
    (type == "object")
    and ((.file_hash_md4_hex? | type) == "string")
    and ((.file_size? | type) == "number")
    and (.file_size > 0)
  )
'

stack_token() {
  local token_file
  token_file="$RUN_DIR/data/api.token"
  [[ -s "$token_file" ]] || return 1
  tr -d '\r\n' <"$token_file"
}

stack_auth_get() {
  local path="$1"
  local token
  token="$(stack_token)" || return 1
  curl -sS -H "Authorization: Bearer $token" "$BASE_URL$path"
}

stack_auth_post() {
  local path="$1"
  local json="$2"
  local token
  token="$(stack_token)" || return 1
  curl -sS -H "Authorization: Bearer $token" -H "content-type: application/json" \
    -d "$json" "$BASE_URL$path"
}

publish_auth_post() {
  local path="$1"
  local json="$2"
  local publish_base_url="$3"
  local publish_token_file="$4"
  local token

  if [[ "$publish_base_url" == "$BASE_URL" && -z "$publish_token_file" ]]; then
    stack_auth_post "$path" "$json"
    return
  fi

  if [[ -z "$publish_token_file" || ! -s "$publish_token_file" ]]; then
    echo "ERROR: publish token file missing or empty: $publish_token_file" >&2
    exit 1
  fi
  token="$(tr -d '\r\n' <"$publish_token_file")"
  curl -sS -H "Authorization: Bearer $token" -H "content-type: application/json" \
    -d "$json" "$publish_base_url$path"
}

publish_fixture_sources_on_stack() {
  local fixture_file rec file_id_hex file_size payload

  if [[ "${FIXTURES_ONLY:-0}" != "1" || "${STACK_PUBLISH_FIXTURES:-1}" != "1" ]]; then
    return 0
  fi
  fixture_file="${DOWNLOAD_FIXTURES_FILE:-}"
  if [[ -z "$fixture_file" || ! -f "$fixture_file" ]]; then
    echo "ERROR: stack fixture publish requires DOWNLOAD_FIXTURES_FILE: $fixture_file" >&2
    exit 1
  fi

  log "fixture-publish-start file=$fixture_file base_url=$STACK_PUBLISH_BASE_URL token_file=${STACK_PUBLISH_TOKEN_FILE:-<stack>}"
  while IFS= read -r rec; do
    [[ -n "$rec" ]] || continue
    file_id_hex="$(printf '%s\n' "$rec" | jq -r '.file_hash_md4_hex')"
    file_size="$(printf '%s\n' "$rec" | jq -r '.file_size')"
    payload="$(jq -nc --arg id "$file_id_hex" --argjson size "$file_size" '{file_id_hex:$id,file_size:$size}')"
    publish_auth_post "/api/v1/kad/publish_source" "$payload" "$STACK_PUBLISH_BASE_URL" "$STACK_PUBLISH_TOKEN_FILE" >/dev/null
    log "fixture-publish file_id=$file_id_hex file_size=$file_size"
  done < <(jq -cr "$valid_fixture_filter" "$fixture_file")
  log "fixture-publish-done file=$fixture_file"
}

wait_for_fixture_sources() {
  local fixture_file rec file_id_hex file_size search_payload start now elapsed last_log sources_json sources_count status_json

  if [[ "${FIXTURES_ONLY:-0}" != "1" ]]; then
    return 0
  fi
  fixture_file="${DOWNLOAD_FIXTURES_FILE:-}"
  if [[ -z "$fixture_file" || ! -f "$fixture_file" ]]; then
    echo "ERROR: FIXTURES_ONLY=1 but DOWNLOAD_FIXTURES_FILE is missing: $fixture_file" >&2
    exit 1
  fi

  while IFS= read -r rec; do
    [[ -n "$rec" ]] || continue
    file_id_hex="$(printf '%s\n' "$rec" | jq -r '.file_hash_md4_hex')"
    file_size="$(printf '%s\n' "$rec" | jq -r '.file_size')"
    search_payload="$(jq -nc --arg id "$file_id_hex" --argjson size "$file_size" '{file_id_hex:$id,file_size:$size}')"
    stack_auth_post "/api/v1/kad/search_sources" "$search_payload" >/dev/null || true

    start="$(ts_epoch)"
    last_log=0
    while true; do
      sources_json="$(stack_auth_get "/api/v1/kad/sources/$file_id_hex" || echo '{}')"
      sources_count="$(printf '%s\n' "$sources_json" | jq -r '(.sources // []) | length' 2>/dev/null || echo 0)"
      if [[ "$sources_count" =~ ^[0-9]+$ ]] && (( sources_count > 0 )); then
        log "fixture-sources-ready file_id=$file_id_hex count=$sources_count"
        break
      fi

      now="$(ts_epoch)"
      elapsed="$((now - start))"
      if (( elapsed - last_log >= PROGRESS_LOG_SECS )); then
        log "fixture-sources-wait file_id=$file_id_hex elapsed=${elapsed}s remaining=$((FIXTURE_SOURCE_TIMEOUT_SECS - elapsed))s count=$sources_count"
        last_log="$elapsed"
      fi
      if (( elapsed > FIXTURE_SOURCE_TIMEOUT_SECS )); then
        status_json="$(stack_auth_get "/api/v1/status" || echo '{}')"
        echo "ERROR: fixture source discovery timeout file_id=$file_id_hex elapsed=${elapsed}s count=$sources_count" >&2
        echo "$status_json" | jq -r '{sent_search_source_reqs,recv_search_source_reqs,source_store_files,source_store_entries_total,live,routing}' >&2 || true
        dump_download_diagnostics "fixture_sources_timeout"
        exit 1
      fi
      sleep "$POLL_SECS"
      stack_auth_post "/api/v1/kad/search_sources" "$search_payload" >/dev/null || true
    done
  done < <(jq -cr "$valid_fixture_filter" "$fixture_file")
}

stack_status_raw() {
  "$STACK_SCRIPT" status 2>&1 || true
}

status_field() {
  local output="$1"
  local key="$2"
  echo "$output" \
    | awk -v k="$key" '
        $0 ~ ("^" k "=") {
          line=$0
          sub("^" k "=", "", line)
          split(line, a, " ")
          print a[1]
        }
      ' \
    | tail -n 1
}

latest_scenario_line() {
  local scenario="$1"
  [[ -f "$STATUS_TSV" ]] || return 1
  awk -F'\t' -v s="$scenario" '$2 == s { last = $0 } END { if (last != "") print last }' "$STATUS_TSV"
}

wait_for_run_dir() {
  local start now elapsed
  start="$(ts_epoch)"
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > HEALTH_TIMEOUT_SECS )); then
      echo "ERROR: timed out waiting for run_dir in $CONTROL_DIR" >&2
      exit 1
    fi
    if [[ -s "$CONTROL_DIR/run_dir" ]]; then
      RUN_DIR="$(cat "$CONTROL_DIR/run_dir")"
      if [[ -d "$RUN_DIR" ]]; then
        STATUS_TSV="$RUN_DIR/soak-band/status.tsv"
        mkdir -p "$RESUME_OUT_DIR"
        return 0
      fi
    fi
    sleep "$POLL_SECS"
  done
}

wait_for_status_file() {
  local start now elapsed
  start="$(ts_epoch)"
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > HEALTH_TIMEOUT_SECS )); then
      echo "ERROR: timed out waiting for status.tsv at $STATUS_TSV" >&2
      exit 1
    fi
    [[ -f "$STATUS_TSV" ]] && return 0
    sleep "$POLL_SECS"
  done
}

wait_for_scenario_running() {
  local scenario="$1"
  local timeout_secs="$2"
  local start now elapsed line status state last_log
  start="$(ts_epoch)"
  last_log=0
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > timeout_secs )); then
      echo "ERROR: timed out waiting for scenario=$scenario to enter running/running" >&2
      exit 1
    fi
    line="$(latest_scenario_line "$scenario" || true)"
    if [[ -n "$line" ]]; then
      status="$(echo "$line" | cut -f3)"
      state="$(echo "$line" | cut -f4)"
      if (( elapsed - last_log >= PROGRESS_LOG_SECS )); then
        log "scenario-wait scenario=$scenario elapsed=${elapsed}s remaining=$((timeout_secs - elapsed))s status=${status:-unknown} state=${state:-unknown}"
        last_log="$elapsed"
      fi
      if [[ "$status" == "running" && "$state" == "running" ]]; then
        log "scenario-ready scenario=$scenario elapsed=${elapsed}s"
        return 0
      fi
      if [[ "$state" == "completed" || "$state" == "failed" || "$state" == "stopped" ]]; then
        echo "ERROR: scenario=$scenario reached terminal state before crash point: $line" >&2
        exit 1
      fi
    fi
    sleep "$POLL_SECS"
  done
}

wait_for_health_code() {
  local wanted="$1"
  local timeout_secs="$2"
  local start now elapsed code
  start="$(ts_epoch)"
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > timeout_secs )); then
      echo "ERROR: health endpoint did not reach code=$wanted within ${timeout_secs}s" >&2
      exit 1
    fi
    code="$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/v1/health" || true)"
    if [[ "$code" == "$wanted" ]]; then
      return 0
    fi
    sleep "$POLL_SECS"
  done
}

wait_for_app_exit_by_pid() {
  local pid="$1"
  local timeout_secs="$2"
  local start now elapsed
  start="$(ts_epoch)"
  while true; do
    if ! is_pid_alive "$pid"; then
      return 0
    fi
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > timeout_secs )); then
      echo "ERROR: app pid=$pid is still alive after ${timeout_secs}s" >&2
      exit 1
    fi
    sleep "$POLL_SECS"
  done
}

count_run_dir_rust_mule_procs() {
  local proc pid cwd cmdline count
  count=0
  for proc in /proc/[0-9]*; do
    [[ -d "$proc" ]] || continue
    pid="${proc##*/}"
    cwd="$(readlink -f "$proc/cwd" 2>/dev/null || true)"
    cmdline="$(tr '\0' ' ' <"$proc/cmdline" 2>/dev/null || true)"
    if [[ "$cwd" == "$RUN_DIR"* && "$cmdline" == *"rust-mule"* ]]; then
      count=$((count + 1))
      continue
    fi
    if [[ "$cmdline" == *"$RUN_DIR"* && "$cmdline" == *"rust-mule"* ]]; then
      count=$((count + 1))
    fi
  done
  echo "$count"
}

list_run_dir_rust_mule_procs() {
  local proc pid cwd cmdline
  for proc in /proc/[0-9]*; do
    [[ -d "$proc" ]] || continue
    pid="${proc##*/}"
    cwd="$(readlink -f "$proc/cwd" 2>/dev/null || true)"
    cmdline="$(tr '\0' ' ' <"$proc/cmdline" 2>/dev/null || true)"
    if [[ "$cwd" == "$RUN_DIR"* && "$cmdline" == *"rust-mule"* ]]; then
      printf '%s\tcwd=%s\tcmd=%s\n' "$pid" "$cwd" "$cmdline"
      continue
    fi
    if [[ "$cmdline" == *"$RUN_DIR"* && "$cmdline" == *"rust-mule"* ]]; then
      printf '%s\tcwd=%s\tcmd=%s\n' "$pid" "$cwd" "$cmdline"
    fi
  done
}

run_dir_rust_mule_pids() {
  local proc pid cwd cmdline
  for proc in /proc/[0-9]*; do
    [[ -d "$proc" ]] || continue
    pid="${proc##*/}"
    cwd="$(readlink -f "$proc/cwd" 2>/dev/null || true)"
    cmdline="$(tr '\0' ' ' <"$proc/cmdline" 2>/dev/null || true)"
    if [[ "$cwd" == "$RUN_DIR"* && "$cmdline" == *"rust-mule"* ]]; then
      echo "$pid"
      continue
    fi
    if [[ "$cmdline" == *"$RUN_DIR"* && "$cmdline" == *"rust-mule"* ]]; then
      echo "$pid"
    fi
  done
}

wait_for_run_dir_processes_gone() {
  local timeout_secs="$1"
  local start now elapsed cnt
  start="$(ts_epoch)"
  while true; do
    cnt="$(count_run_dir_rust_mule_procs)"
    if [[ "$cnt" == "0" ]]; then
      return 0
    fi
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > timeout_secs )); then
      echo "ERROR: run-dir rust-mule processes still present after ${timeout_secs}s (count=$cnt)" >&2
      list_run_dir_rust_mule_procs >&2 || true
      exit 1
    fi
    sleep "$POLL_SECS"
  done
}

api_token() {
  cat "$RUN_DIR/data/api.token"
}

poll_downloads_json() {
  local out="$1"
  local token
  token="$(api_token)"
  curl -sS -H "Authorization: Bearer $token" "$BASE_URL/api/v1/downloads" >"$out"
}

dump_download_diagnostics() {
  local tag="$1"
  local downloads_json status_json
  downloads_json="$RESUME_OUT_DIR/${tag}_downloads_diag.json"
  status_json="$RESUME_OUT_DIR/${tag}_status_diag.json"

  poll_downloads_json "$downloads_json" || true
  stack_auth_get "/api/v1/status" >"$status_json" 2>/dev/null || echo '{}' >"$status_json"

  log "diag-dump tag=$tag downloads_json=$downloads_json status_json=$status_json"
  jq -r '
    {
      queue_len,
      reserve_calls_total,
      reserve_granted_blocks_total,
      reserve_denied_cooldown_total,
      reserve_denied_peer_cap_total,
      reserve_denied_download_cap_total,
      reserve_denied_state_total,
      reserve_empty_no_missing_total,
      downloads_count: ((.downloads // []) | length),
      inflight_total: (((.downloads // []) | map(.inflight_ranges // 0) | add) // 0),
      downloaded_total: (((.downloads // []) | map(.downloaded_bytes // 0) | add) // 0),
      states: ((.downloads // []) | group_by(.state) | map({state: .[0].state, count: length}))
    }
  ' "$downloads_json" 2>/dev/null || true
}

snapshot_downloads() {
  local label="$1"
  poll_downloads_json "$RESUME_OUT_DIR/${label}_downloads.json"

  {
    echo "timestamp=$(ts)"
    echo "run_dir=$RUN_DIR"
    echo "label=$label"
    echo "downloads_count=$(jq -r '(.downloads // []) | length' "$RESUME_OUT_DIR/${label}_downloads.json")"
    echo "queue_len=$(jq -r '.queue_len // 0' "$RESUME_OUT_DIR/${label}_downloads.json")"
    echo "part_files=$(find "$RUN_DIR/data/download" -maxdepth 1 -type f -name '*.part' | wc -l)"
    echo "part_met_files=$(find "$RUN_DIR/data/download" -maxdepth 1 -type f -name '*.part.met' | wc -l)"
    echo "part_met_bak_files=$(find "$RUN_DIR/data/download" -maxdepth 1 -type f -name '*.part.met.bak' | wc -l)"
  } >"$RESUME_OUT_DIR/${label}_summary.txt"

  jq -r '
    [
      (.downloads // [])[] |
      select(
        (.downloaded_bytes > .file_size)
        or (.progress_pct < 0 or .progress_pct > 100)
        or (.missing_ranges < 0)
        or (.inflight_ranges < 0)
        or (.retry_count < 0)
        or ((.state | IN("queued","downloading","paused","cancelled","completed","error")) | not)
      )
    ] | length
  ' "$RESUME_OUT_DIR/${label}_downloads.json" >"$RESUME_OUT_DIR/${label}_violations.count"
}

has_active_transfer_in_file() {
  local file="$1"
  jq -e '
    ((.downloads // []) | length) > 0
    and any(.downloads[]?; ((.downloaded_bytes // 0) > 0) and ((.inflight_ranges // 0) > 0))
  ' "$file" >/dev/null 2>&1
}

wait_for_active_transfer() {
  local timeout_secs="$1"
  local start now elapsed last_log tmp_json downloads_count reserve_calls reserve_granted
  tmp_json="$RESUME_OUT_DIR/active_probe.json"
  start="$(ts_epoch)"
  last_log=0
  while true; do
    poll_downloads_json "$tmp_json"
    if has_active_transfer_in_file "$tmp_json"; then
      log "active-transfer detected (downloaded_bytes>0 && inflight_ranges>0)"
      return 0
    fi
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    downloads_count="$(jq -r '(.downloads // []) | length' "$tmp_json" 2>/dev/null || echo 0)"
    reserve_calls="$(jq -r '.reserve_calls_total // 0' "$tmp_json" 2>/dev/null || echo 0)"
    reserve_granted="$(jq -r '.reserve_granted_blocks_total // 0' "$tmp_json" 2>/dev/null || echo 0)"

    if (( elapsed - last_log >= PROGRESS_LOG_SECS )); then
      log "active-transfer-wait elapsed=${elapsed}s remaining=$((timeout_secs - elapsed))s downloads=$downloads_count reserve_calls_total=$reserve_calls reserve_granted_blocks_total=$reserve_granted"
      last_log="$elapsed"
    fi
    if (( elapsed > NO_RESERVE_ACTIVITY_TIMEOUT_SECS )) \
      && [[ "$downloads_count" =~ ^[0-9]+$ ]] \
      && [[ "$reserve_calls" =~ ^[0-9]+$ ]] \
      && (( downloads_count > 0 )) \
      && (( reserve_calls == 0 )); then
      echo "ERROR: no reserve activity observed within ${NO_RESERVE_ACTIVITY_TIMEOUT_SECS}s (downloads=$downloads_count reserve_calls_total=$reserve_calls)" >&2
      dump_download_diagnostics "no_reserve_activity"
      exit 1
    fi
    if (( elapsed > timeout_secs )); then
      echo "ERROR: no active transfer observed within ${timeout_secs}s" >&2
      jq -r '
        "downloads=\((.downloads // []) | length) total_downloaded=\(((.downloads // []) | map(.downloaded_bytes // 0) | add) // 0) inflight_total=\(((.downloads // []) | map(.inflight_ranges // 0) | add) // 0)"
      ' "$tmp_json" >&2 || true
      dump_download_diagnostics "active_transfer_timeout"
      exit 1
    fi
    sleep "$POLL_SECS"
  done
}

assert_monotonic_download_bytes() {
  local pre="$RESUME_OUT_DIR/pre_downloads.json"
  local post="$RESUME_OUT_DIR/post_downloads.json"
  local out="$RESUME_OUT_DIR/post_monotonic_violations.txt"

  jq -nr --argfile pre "$pre" --argfile post "$post" '
    def dl_key($d):
      if ($d.part_number? != null) then ("part:" + (($d.part_number | tostring)))
      elif ($d.id? != null) then ("id:" + ($d.id | tostring))
      elif ($d.file_hash_md4_hex? != null) then ("hash:" + ($d.file_hash_md4_hex | tostring))
      else empty
      end;
    def by_key($arr):
      reduce ($arr[]?) as $d ({}; (dl_key($d)) as $k | if $k == null or $k == "" then . else .[$k] = ($d.downloaded_bytes // 0) end);
    ($pre.downloads // []) as $pre_dl
    | ($post.downloads // []) as $post_dl
    | (by_key($pre_dl)) as $pre_map
    | (by_key($post_dl)) as $post_map
    | ($pre_map | to_entries[])
    | select((($post_map[.key] // -1) < .value))
    | "\(.key)\tpre=\(.value)\tpost=\($post_map[.key] // -1)"
  ' >"$out"

  if [[ -s "$out" ]]; then
    echo "ERROR: post-restart downloaded_bytes regressed for one or more downloads" >&2
    cat "$out" >&2
    exit 1
  fi
  log "monotonic check passed (post downloaded_bytes >= pre for all pre-existing downloads)"
}

wait_for_any_completed_download() {
  local timeout_secs="$1"
  local start now elapsed last_log tmp_json
  tmp_json="$RESUME_OUT_DIR/completion_probe.json"
  start="$(ts_epoch)"
  last_log=0
  while true; do
    poll_downloads_json "$tmp_json"
    if jq -e '
      any(.downloads[]?; (.state == "completed") or (((.file_size // 0) > 0) and ((.downloaded_bytes // 0) >= (.file_size // 0))))
    ' "$tmp_json" >/dev/null 2>&1; then
      log "completion gate passed (>=1 completed download observed)"
      return 0
    fi
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed - last_log >= PROGRESS_LOG_SECS )); then
      log "completion-wait elapsed=${elapsed}s remaining=$((timeout_secs - elapsed))s downloads=$(jq -r '(.downloads // []) | length' "$tmp_json" 2>/dev/null || echo 0)"
      last_log="$elapsed"
    fi
    if (( elapsed > timeout_secs )); then
      echo "ERROR: no completed download observed within ${timeout_secs}s after restart" >&2
      jq -r '
        "downloads=\((.downloads // []) | length) completed=\(((.downloads // []) | map(select(.state=="completed")) | length)) total_downloaded=\(((.downloads // []) | map(.downloaded_bytes // 0) | add) // 0)"
      ' "$tmp_json" >&2 || true
      dump_download_diagnostics "completion_timeout"
      exit 1
    fi
    sleep "$POLL_SECS"
  done
}

crash_app() {
  local app_pid killed_any pid
  app_pid="$(cat "$CONTROL_DIR/app.pid" 2>/dev/null || true)"
  killed_any=0

  # Kill tracked pid first if present.
  if [[ -n "$app_pid" ]] && is_pid_alive "$app_pid"; then
    kill -9 "$app_pid" 2>/dev/null || true
    killed_any=1
  fi

  # Then kill all run-dir rust-mule processes (covers wrapper-pid mismatch).
  while IFS= read -r pid; do
    [[ -n "$pid" ]] || continue
    kill -9 "$pid" 2>/dev/null || true
    killed_any=1
  done < <(run_dir_rust_mule_pids)

  if [[ "$killed_any" != "1" ]]; then
    echo "ERROR: no run-dir rust-mule process found to crash" >&2
    list_run_dir_rust_mule_procs >&2 || true
    exit 1
  fi

  CRASH_EPOCH="$(ts_epoch)"
  echo "${app_pid:-unknown}" >"$RESUME_OUT_DIR/crashed_app.pid"
  log "crash requested tracked_pid=${app_pid:-none}"

  if [[ -n "$app_pid" ]]; then
    wait_for_app_exit_by_pid "$app_pid" "$HEALTH_TIMEOUT_SECS"
  fi
  wait_for_run_dir_processes_gone "$HEALTH_TIMEOUT_SECS"

  # Health may remain up if some unrelated process already serves the port.
  # We no longer require code=000; process-level stop verification is authoritative.
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/v1/health" || true)"
  log "post-crash health status_code=$code (informational)"
}

restart_app() {
  local new_pid
  local existing
  existing="$(count_run_dir_rust_mule_procs)"
  if [[ "$existing" != "0" ]]; then
    echo "ERROR: refusing restart; run-dir rust-mule process(es) still alive count=$existing" >&2
    list_run_dir_rust_mule_procs >&2 || true
    exit 1
  fi
  nohup bash -lc "cd '$RUN_DIR' && ./rust-mule" >"$RUN_DIR/rust-mule.resume.out" 2>&1 &
  new_pid="$!"
  echo "$new_pid" >"$CONTROL_DIR/app.pid"
  RESTART_EPOCH="$(ts_epoch)"
  echo "$new_pid" >"$RESUME_OUT_DIR/restarted_app.pid"
  log "restarted app pid=$new_pid"

  sleep 1
  if ! is_pid_alive "$new_pid"; then
    echo "ERROR: restarted app exited immediately pid=$new_pid" >&2
    tail -n 120 "$RUN_DIR/rust-mule.resume.out" >&2 || true
    exit 1
  fi
}

wait_for_post_restart_progress() {
  local scenario="$1"
  local timeout_secs="$2"
  local start now elapsed baseline_lines current_lines line status state last_log

  baseline_lines="$(wc -l <"$STATUS_TSV" 2>/dev/null || echo 0)"
  start="$(ts_epoch)"
  last_log=0
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > timeout_secs )); then
      echo "ERROR: timed out waiting for post-restart scenario progress" >&2
      exit 1
    fi

    current_lines="$(wc -l <"$STATUS_TSV" 2>/dev/null || echo 0)"
    line="$(latest_scenario_line "$scenario" || true)"
    if [[ -n "$line" ]]; then
      status="$(echo "$line" | cut -f3)"
      state="$(echo "$line" | cut -f4)"
      if (( elapsed - last_log >= PROGRESS_LOG_SECS )); then
        log "post-restart-wait scenario=$scenario elapsed=${elapsed}s remaining=$((timeout_secs - elapsed))s status=${status:-unknown} state=${state:-unknown} lines=$current_lines"
        last_log="$elapsed"
      fi
      if (( current_lines > baseline_lines + 2 )) && [[ "$status" == "running" || "$state" == "completed" ]]; then
        log "post-restart-ready scenario=$scenario elapsed=${elapsed}s lines=$current_lines"
        return 0
      fi
      if [[ "$state" == "failed" ]]; then
        echo "ERROR: scenario failed after restart: $line" >&2
        exit 1
      fi
    fi
    sleep "$POLL_SECS"
  done
}

wait_for_stack_terminal() {
  local start now elapsed out runner_state last_log
  start="$(ts_epoch)"
  last_log=0
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > WAIT_TIMEOUT_SECS )); then
      echo "ERROR: timed out waiting for stack terminal state" >&2
      return 1
    fi
    out="$(stack_status_raw)"
    runner_state="$(status_field "$out" "runner_state")"
    if (( elapsed - last_log >= PROGRESS_LOG_SECS )); then
      log "stack-terminal-wait elapsed=${elapsed}s remaining=$((WAIT_TIMEOUT_SECS - elapsed))s runner_state=${runner_state:-unknown}"
      last_log="$elapsed"
    fi
    case "${runner_state:-unknown}" in
    completed) return 0 ;;
    failed | stopped) return 1 ;;
    esac
    sleep "$POLL_SECS"
  done
}

collect_stack_bundle() {
  STACK_TARBALL="$("$STACK_SCRIPT" collect)"
  echo "$STACK_TARBALL" >"$RESUME_OUT_DIR/stack_bundle.path"
  log "collected stack bundle $STACK_TARBALL"
}

write_report() {
  local out runner_state
  out="$(stack_status_raw)"
  runner_state="$(status_field "$out" "runner_state")"

  {
    echo "timestamp=$(ts)"
    echo "stack_script=$STACK_SCRIPT"
    echo "stack_root=$STACK_ROOT"
    echo "run_dir=$RUN_DIR"
    echo "scenario=$RESUME_SCENARIO"
    echo "base_url=$BASE_URL"
    echo "crash_epoch=$CRASH_EPOCH"
    echo "restart_epoch=$RESTART_EPOCH"
    echo "runner_state=${runner_state:-unknown}"
    echo "stack_tarball=${STACK_TARBALL:-none}"
    echo "pre_violations=$(cat "$RESUME_OUT_DIR/pre_violations.count" 2>/dev/null || echo unknown)"
    echo "post_violations=$(cat "$RESUME_OUT_DIR/post_violations.count" 2>/dev/null || echo unknown)"
  } >"$RESUME_OUT_DIR/resume_report.txt"
}

run_resume_soak() {
  local out status
  require_tools
  mkdir -p "$RESUME_OUT_DIR"
  trap cleanup_on_exit EXIT INT TERM

  out="$(stack_status_raw)"
  status="$(status_field "$out" "status")"
  if [[ "$status" == "running" ]]; then
    echo "ERROR: stack runner already running; stop it first or use separate STACK_ROOT." >&2
    exit 1
  fi

  log "starting stack soak via $STACK_SCRIPT"
  BASE_URL="$STACK_BASE_URL" \
    API_PORT="$STACK_API_PORT" \
    DOWNLOAD_FIXTURES_FILE="${DOWNLOAD_FIXTURES_FILE:-}" \
    FIXTURES_ONLY="${FIXTURES_ONLY:-0}" \
    "$STACK_SCRIPT" start
  STACK_STARTED=1

  wait_for_run_dir
  wait_for_status_file
  log "run_dir=$RUN_DIR status_tsv=$STATUS_TSV"

  publish_fixture_sources_on_stack
  wait_for_scenario_running "$RESUME_SCENARIO" "$WAIT_TIMEOUT_SECS"
  wait_for_fixture_sources
  wait_for_active_transfer "$ACTIVE_TRANSFER_TIMEOUT_SECS"
  snapshot_downloads "pre"

  crash_app

  restart_app
  wait_for_health_code "200" "$HEALTH_TIMEOUT_SECS"
  snapshot_downloads "post"
  assert_monotonic_download_bytes

  wait_for_post_restart_progress "$RESUME_SCENARIO" "$HEALTH_TIMEOUT_SECS"
  wait_for_any_completed_download "$COMPLETION_TIMEOUT_SECS"

  if ! wait_for_stack_terminal; then
    log "stack terminal state is non-completed; requesting stop"
    "$STACK_SCRIPT" stop >/dev/null 2>&1 || true
  fi

  collect_stack_bundle
  write_report
  STOP_ON_EXIT=0
  log "resume-soak complete report=$RESUME_OUT_DIR/resume_report.txt"
}

case "${1:-run}" in
run)
  run_resume_soak
  ;;
*)
  echo "usage: scripts/test/download_resume_soak.sh [run]"
  exit 2
  ;;
esac
