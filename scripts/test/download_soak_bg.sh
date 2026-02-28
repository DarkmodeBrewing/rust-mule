#!/usr/bin/env bash
set -euo pipefail

# Generic timed background soak harness for download API scenarios.
#
# Usage:
#   SCENARIO=<single_e2e|long_churn|integrity|concurrency> \
#     scripts/test/download_soak_bg.sh start [duration_secs]
#   ... run|status|stop|collect
#
# Environment:
#   SCENARIO=single_e2e
#   BASE_URL=http://127.0.0.1:17835
#   TOKEN_FILE=data/api.token
#   RUN_ROOT=/tmp/rust-mule-download-soak
#   WAIT_BETWEEN=5
#   READY_TIMEOUT_SECS=300
#   READY_PATH=/api/v1/downloads
#   READY_HTTP_CODES=200
#   CONCURRENCY_TARGET=20
#   CHURN_MAX_QUEUE=25
#   API_CONNECT_TIMEOUT_SECS=3
#   API_MAX_TIME_SECS=8
#   DOWNLOAD_FIXTURES_FILE=scripts/test/download_fixtures.example.json
#   FIXTURES_ONLY=0

SCENARIO="${SCENARIO:-}"
BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"
RUN_ROOT_BASE="${RUN_ROOT:-/tmp/rust-mule-download-soak}"
WAIT_BETWEEN="${WAIT_BETWEEN:-5}"
READY_TIMEOUT_SECS="${READY_TIMEOUT_SECS:-300}"
READY_PATH="${READY_PATH:-/api/v1/downloads}"
READY_HTTP_CODES="${READY_HTTP_CODES:-200}"
CONCURRENCY_TARGET="${CONCURRENCY_TARGET:-20}"
CHURN_MAX_QUEUE="${CHURN_MAX_QUEUE:-25}"
API_CONNECT_TIMEOUT_SECS="${API_CONNECT_TIMEOUT_SECS:-3}"
API_MAX_TIME_SECS="${API_MAX_TIME_SECS:-8}"
DOWNLOAD_FIXTURES_FILE="${DOWNLOAD_FIXTURES_FILE:-}"
FIXTURES_ONLY="${FIXTURES_ONLY:-0}"

if [[ -z "$SCENARIO" ]]; then
  echo "ERROR: SCENARIO is required (single_e2e|long_churn|integrity|concurrency)" >&2
  exit 2
fi

RUN_ROOT="${RUN_ROOT_BASE}/${SCENARIO}"
LOG_DIR="$RUN_ROOT/logs"
RUNNER_PID_FILE="$RUN_ROOT/runner.pid"
RUNNER_LOG_FILE="$LOG_DIR/runner.log"
RUNNER_STDOUT_FILE="$LOG_DIR/runner.out"
RUNNER_STATE_FILE="$RUN_ROOT/runner.state"
STOP_FILE="$RUN_ROOT/stop.requested"
FAIL_FILE="$RUN_ROOT/failed.flag"
FIXTURE_INDEX_FILE="$RUN_ROOT/fixture.index"
CREATE_FAIL_STREAK_FILE="$RUN_ROOT/create_fail_streak"
FIXTURE_COUNT=0
FIXTURE_NEXT=0
CREATE_FAIL_STREAK=0
CREATE_FAIL_LIMIT="${CREATE_FAIL_LIMIT:-10}"

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
ts_epoch() { date +%s; }
log() { echo "$(ts) $*" | tee -a "$RUNNER_LOG_FILE"; }
clip_detail() {
  local text="${1:-}"
  text="$(echo "$text" | tr '\r\n\t' '   ')"
  printf '%.160s' "$text"
}

ensure_dirs() {
  mkdir -p "$RUN_ROOT" "$LOG_DIR"
}

is_pid_alive() {
  local pid="$1"
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

rand_hex16() { hexdump -n 16 -e '16/1 "%02x"' /dev/urandom; }
rand_md4_hex() { hexdump -n 16 -e '16/1 "%02x"' /dev/urandom; }

auth_header() {
  local token
  token="$(cat "$TOKEN_FILE" 2>/dev/null || true)"
  if [[ -z "$token" ]]; then
    return 1
  fi
  echo "Authorization: Bearer $token"
}

fixtures_path() {
  if [[ -z "$DOWNLOAD_FIXTURES_FILE" ]]; then
    return 1
  fi
  if [[ -f "$DOWNLOAD_FIXTURES_FILE" ]]; then
    printf '%s' "$DOWNLOAD_FIXTURES_FILE"
    return 0
  fi
  if [[ -f "$PWD/$DOWNLOAD_FIXTURES_FILE" ]]; then
    printf '%s' "$PWD/$DOWNLOAD_FIXTURES_FILE"
    return 0
  fi
  return 1
}

load_fixtures() {
  local f
  CREATE_FAIL_STREAK=0
  printf '0\n' >"$CREATE_FAIL_STREAK_FILE"
  if ! f="$(fixtures_path)"; then
    FIXTURE_COUNT=0
    FIXTURE_NEXT=0
    return 0
  fi

  FIXTURE_COUNT="$(jq -r '
    def valid_fixture:
      (type == "object")
      and ((.file_name? | type) == "string")
      and ((.file_hash_md4_hex? | type) == "string")
      and ((.file_size? | type) == "number")
      and (.file_size > 0);
    if type == "array" then
      [ .[] | select(valid_fixture) ] | length
    else 0 end
  ' "$f" 2>/dev/null || echo 0)"

  if ! [[ "$FIXTURE_COUNT" =~ ^[0-9]+$ ]]; then
    FIXTURE_COUNT=0
  fi

  if (( FIXTURE_COUNT > 0 )); then
    log "fixtures loaded file=$f count=$FIXTURE_COUNT"
  else
    log "fixtures not loaded or empty file=$f"
  fi

  if [[ -f "$FIXTURE_INDEX_FILE" ]]; then
    FIXTURE_NEXT="$(cat "$FIXTURE_INDEX_FILE" 2>/dev/null || echo 0)"
  fi
  if ! [[ "$FIXTURE_NEXT" =~ ^[0-9]+$ ]]; then
    FIXTURE_NEXT=0
  fi
  if (( FIXTURE_COUNT > 0 )); then
    FIXTURE_NEXT="$(( FIXTURE_NEXT % FIXTURE_COUNT ))"
  else
    FIXTURE_NEXT=0
  fi
}

next_fixture_record() {
  local f idx
  if (( FIXTURE_COUNT <= 0 )); then
    return 1
  fi
  f="$(fixtures_path)" || return 1
  idx="$FIXTURE_NEXT"
  jq -cr --argjson idx "$idx" '
    def valid_fixture:
      (type == "object")
      and ((.file_name? | type) == "string")
      and ((.file_hash_md4_hex? | type) == "string")
      and ((.file_size? | type) == "number")
      and (.file_size > 0);
    [ .[] | select(valid_fixture) ] | .[$idx]
  ' "$f" 2>/dev/null || return 1
}

advance_fixture_index() {
  if (( FIXTURE_COUNT <= 0 )); then
    return 0
  fi
  FIXTURE_NEXT="$(( (FIXTURE_NEXT + 1) % FIXTURE_COUNT ))"
  printf '%s\n' "$FIXTURE_NEXT" >"$FIXTURE_INDEX_FILE"
}

create_download() {
  local fallback_name="$1"
  local fallback_size="$2"
  local fallback_md4="$3"
  local rec name size md4 resp source_label part err_code err_msg err_excerpt

  source_label="fallback"
  if rec="$(next_fixture_record)"; then
    name="$(echo "$rec" | jq -r '.file_name')"
    size="$(echo "$rec" | jq -r '.file_size')"
    md4="$(echo "$rec" | jq -r '.file_hash_md4_hex')"
    source_label="fixture"
    resp="$(downloads_create "$name" "$size" "$md4" || true)"
    advance_fixture_index
  else
    if [[ "$FIXTURES_ONLY" == "1" ]]; then
      log "ERROR: fixtures_only enabled but no valid fixture available"
      touch "$FAIL_FILE"
      return 1
    fi
    resp="$(downloads_create "$fallback_name" "$fallback_size" "$fallback_md4" || true)"
  fi

  part="$(echo "$resp" | jq -r '.download.part_number // empty' 2>/dev/null || true)"
  if [[ -n "$part" ]]; then
    CREATE_FAIL_STREAK=0
    printf '%s\n' "$CREATE_FAIL_STREAK" >"$CREATE_FAIL_STREAK_FILE"
    echo "$resp"
    return 0
  fi

  if [[ -f "$CREATE_FAIL_STREAK_FILE" ]]; then
    local persisted_streak
    persisted_streak="$(tr -d ' \t\r\n' <"$CREATE_FAIL_STREAK_FILE" 2>/dev/null || true)"
    if [[ "$persisted_streak" =~ ^[0-9]+$ ]]; then
      CREATE_FAIL_STREAK="$persisted_streak"
    fi
  fi
  CREATE_FAIL_STREAK="$((CREATE_FAIL_STREAK + 1))"
  printf '%s\n' "$CREATE_FAIL_STREAK" >"$CREATE_FAIL_STREAK_FILE"
  err_code="$(echo "$resp" | jq -r '.error.code // .code // empty' 2>/dev/null || true)"
  err_msg="$(echo "$resp" | jq -r '.error.message // .message // empty' 2>/dev/null || true)"
  err_excerpt="$(clip_detail "$resp")"
  log "WARN: create returned no part source=$source_label streak=$CREATE_FAIL_STREAK error_code=${err_code:-none} error_message=${err_msg:-none} response=${err_excerpt:-empty}"
  if [[ "$FIXTURES_ONLY" == "1" ]] && (( CREATE_FAIL_STREAK >= CREATE_FAIL_LIMIT )); then
    log "ERROR: repeated create failures in fixtures-only mode streak=$CREATE_FAIL_STREAK limit=$CREATE_FAIL_LIMIT"
    touch "$FAIL_FILE"
  fi
  echo "$resp"
}

api_get() {
  local path="$1"
  local auth
  auth="$(auth_header)" || return 1
  curl -sS \
    --connect-timeout "$API_CONNECT_TIMEOUT_SECS" \
    --max-time "$API_MAX_TIME_SECS" \
    -H "$auth" \
    "$BASE_URL$path"
}

api_post() {
  local path="$1"
  local json="${2:-{}}"
  local auth
  auth="$(auth_header)" || return 1
  curl -sS \
    --connect-timeout "$API_CONNECT_TIMEOUT_SECS" \
    --max-time "$API_MAX_TIME_SECS" \
    -H "$auth" \
    -H "content-type: application/json" \
    -d "$json" \
    "$BASE_URL$path"
}

api_delete() {
  local path="$1"
  local auth
  auth="$(auth_header)" || return 1
  curl -sS \
    --connect-timeout "$API_CONNECT_TIMEOUT_SECS" \
    --max-time "$API_MAX_TIME_SECS" \
    -X DELETE \
    -H "$auth" \
    "$BASE_URL$path"
}

downloads_list() {
  api_get "/api/v1/downloads"
}

downloads_part_numbers() {
  downloads_list | jq -r '.downloads[].part_number'
}

downloads_random_part() {
  local parts picked
  parts="$(downloads_part_numbers | tr '\n' ' ')"
  set -- $parts
  if [[ $# -eq 0 ]]; then
    echo ""
    return 0
  fi
  picked="$(( (RANDOM % $#) + 1 ))"
  eval "echo \${$picked}"
}

downloads_create() {
  local name="$1"
  local size="$2"
  local md4="$3"
  local payload
  payload="$(jq -nc \
    --arg file_name "$name" \
    --arg file_hash_md4_hex "$md4" \
    --argjson file_size "$size" \
    '{file_name:$file_name,file_size:$file_size,file_hash_md4_hex:$file_hash_md4_hex}')" || return 1
  api_post "/api/v1/downloads" "$payload"
}

downloads_pause() { local part="$1"; api_post "/api/v1/downloads/$part/pause" "{}"; }
downloads_resume() { local part="$1"; api_post "/api/v1/downloads/$part/resume" "{}"; }
downloads_cancel() { local part="$1"; api_post "/api/v1/downloads/$part/cancel" "{}"; }
downloads_delete() { local part="$1"; api_delete "/api/v1/downloads/$part"; }

wait_ready() {
  local start now elapsed code auth
  start="$(ts_epoch)"
  while true; do
    now="$(ts_epoch)"
    elapsed="$(( now - start ))"
    if (( elapsed > READY_TIMEOUT_SECS )); then
      log "ERROR: readiness timeout after ${READY_TIMEOUT_SECS}s"
      return 1
    fi
    auth="$(auth_header)" || {
      log "ready-check elapsed=${elapsed}s token=missing"
      sleep 2
      continue
    }
    code="$(curl -s \
      --connect-timeout "$API_CONNECT_TIMEOUT_SECS" \
      --max-time "$API_MAX_TIME_SECS" \
      -o /dev/null \
      -w '%{http_code}' \
      -H "$auth" \
      "$BASE_URL$READY_PATH" || true)"
    log "ready-check elapsed=${elapsed}s path=$READY_PATH status_code=$code ok_codes=$READY_HTTP_CODES"
    if [[ ",$READY_HTTP_CODES," == *",$code,"* ]]; then
      return 0
    fi
    sleep 3
  done
}

write_round_row() {
  local round="$1"
  local action="$2"
  local detail="$3"
  local list_json queue_len
  list_json="$(downloads_list || echo '{}')"
  queue_len="$(echo "$list_json" | jq -r '.queue_len // 0' 2>/dev/null || echo 0)"
  printf '%s\t%s\t%s\t%s\t%s\n' "$(ts)" "$round" "$action" "$queue_len" "$detail" >>"$LOG_DIR/rounds.tsv"
  printf '%s\n' "$list_json" >>"$LOG_DIR/list.ndjson"
}

scenario_single_e2e_round() {
  local round="$1"
  local part state

  part="$(downloads_random_part)"
  if [[ -z "$part" ]]; then
    local name size md4 create_json create_err
    name="single-e2e-${round}.bin"
    size="$(( (RANDOM % 20000000) + 1000000 ))"
    md4="$(rand_md4_hex)"
    create_json="$(create_download "$name" "$size" "$md4" || true)"
    part="$(echo "$create_json" | jq -r '.download.part_number // empty' 2>/dev/null || true)"
    if [[ -z "$part" ]]; then
      create_err="$(echo "$create_json" | jq -r '.error.message // .error.code // "unknown"' 2>/dev/null || echo unknown)"
      write_round_row "$round" "create" "part=none error=$(clip_detail "$create_err")"
    else
      write_round_row "$round" "create" "part=$part"
    fi
    return 0
  fi

  state="$(downloads_list | jq -r ".downloads[] | select(.part_number==$part) | .state" | head -n1)"
  if [[ "$state" == "paused" ]]; then
    downloads_resume "$part" >/dev/null || true
    write_round_row "$round" "resume" "part=$part"
  else
    downloads_pause "$part" >/dev/null || true
    write_round_row "$round" "pause" "part=$part"
  fi

  if (( round % 20 == 0 )); then
    downloads_cancel "$part" >/dev/null || true
    downloads_delete "$part" >/dev/null || true
    write_round_row "$round" "cleanup" "part=$part"
  fi
}

scenario_long_churn_round() {
  local round part action queue_len
  local name size md4 create_json create_part create_err
  round="$1"

  name="churn-${round}-$(rand_hex16).bin"
  size="$(( (RANDOM % 10000000) + 500000 ))"
  md4="$(rand_md4_hex)"
  create_json="$(create_download "$name" "$size" "$md4" || true)"
  create_part="$(echo "$create_json" | jq -r '.download.part_number // empty' 2>/dev/null || true)"
  if [[ -z "$create_part" ]]; then
    create_err="$(echo "$create_json" | jq -r '.error.message // .error.code // "unknown"' 2>/dev/null || echo unknown)"
    write_round_row "$round" "create_fail" "error=$(clip_detail "$create_err")"
  fi

  part="$(downloads_random_part)"
  if [[ -n "$part" ]]; then
    action="$((RANDOM % 4))"
    case "$action" in
    0) downloads_pause "$part" >/dev/null || true; write_round_row "$round" "pause" "part=$part" ;;
    1) downloads_resume "$part" >/dev/null || true; write_round_row "$round" "resume" "part=$part" ;;
    2) downloads_cancel "$part" >/dev/null || true; write_round_row "$round" "cancel" "part=$part" ;;
    3) downloads_delete "$part" >/dev/null || true; write_round_row "$round" "delete" "part=$part" ;;
    esac
  else
    write_round_row "$round" "create" "part=none"
  fi

  queue_len="$(downloads_list | jq -r '.queue_len // 0' 2>/dev/null || echo 0)"
  while (( queue_len > CHURN_MAX_QUEUE )); do
    part="$(downloads_random_part)"
    [[ -z "$part" ]] && break
    downloads_cancel "$part" >/dev/null || true
    downloads_delete "$part" >/dev/null || true
    queue_len="$(downloads_list | jq -r '.queue_len // 0' 2>/dev/null || echo 0)"
  done
}

scenario_integrity_round() {
  local round list_json violations
  local name size md4 create_json create_part create_err
  round="$1"

  if (( round <= 3 )); then
    name="integrity-${round}.bin"
    size="$(( (RANDOM % 5000000) + 2000000 ))"
    md4="$(rand_md4_hex)"
    create_json="$(create_download "$name" "$size" "$md4" || true)"
    create_part="$(echo "$create_json" | jq -r '.download.part_number // empty' 2>/dev/null || true)"
    if [[ -z "$create_part" ]]; then
      create_err="$(echo "$create_json" | jq -r '.error.message // .error.code // "unknown"' 2>/dev/null || echo unknown)"
      write_round_row "$round" "create_fail" "error=$(clip_detail "$create_err")"
    fi
  fi

  list_json="$(downloads_list || echo '{}')"
  violations="$(echo "$list_json" | jq -r '
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
  ' 2>/dev/null || echo 999)"

  if [[ "$violations" != "0" ]]; then
    log "ERROR: integrity_violations count=$violations"
    touch "$FAIL_FILE"
  fi

  local dup_parts
  dup_parts="$(echo "$list_json" | jq -r '
    [(.downloads // [])[].part_number] as $p |
    ($p | length) - ($p | unique | length)
  ' 2>/dev/null || echo 999)"
  if [[ "$dup_parts" != "0" ]]; then
    log "ERROR: duplicate_part_numbers count=$dup_parts"
    touch "$FAIL_FILE"
  fi

  write_round_row "$round" "integrity_check" "violations=$violations dup_parts=$dup_parts"
}

scenario_concurrency_round() {
  local round queue_len i target name size md4 part create_json create_part create_err
  round="$1"
  target="$CONCURRENCY_TARGET"
  queue_len="$(downloads_list | jq -r '.queue_len // 0' 2>/dev/null || echo 0)"

  while (( queue_len < target )); do
    name="concurrency-${round}-$(rand_hex16).bin"
    size="$(( (RANDOM % 15000000) + 500000 ))"
    md4="$(rand_md4_hex)"
    create_json="$(create_download "$name" "$size" "$md4" || true)"
    create_part="$(echo "$create_json" | jq -r '.download.part_number // empty' 2>/dev/null || true)"
    if [[ -z "$create_part" ]]; then
      create_err="$(echo "$create_json" | jq -r '.error.message // .error.code // "unknown"' 2>/dev/null || echo unknown)"
      write_round_row "$round" "create_fail" "error=$(clip_detail "$create_err")"
    fi
    queue_len="$((queue_len + 1))"
  done

  for i in 1 2 3 4 5; do
    part="$(downloads_random_part)"
    [[ -z "$part" ]] && break
    case $((RANDOM % 3)) in
    0) downloads_pause "$part" >/dev/null || true ;;
    1) downloads_resume "$part" >/dev/null || true ;;
    2) downloads_cancel "$part" >/dev/null || true ;;
    esac
  done

  if (( round % 10 == 0 )); then
    for i in 1 2 3; do
      part="$(downloads_random_part)"
      [[ -z "$part" ]] && break
      downloads_cancel "$part" >/dev/null || true
      downloads_delete "$part" >/dev/null || true
    done
  fi

  write_round_row "$round" "concurrency_tick" "target=$target"
}

run_round() {
  local round="$1"
  case "$SCENARIO" in
  single_e2e) scenario_single_e2e_round "$round" ;;
  long_churn) scenario_long_churn_round "$round" ;;
  integrity) scenario_integrity_round "$round" ;;
  concurrency) scenario_concurrency_round "$round" ;;
  *)
    log "ERROR: unsupported scenario=$SCENARIO"
    touch "$FAIL_FILE"
    return 1
    ;;
  esac
}

run_foreground() {
  local duration_secs="${1:-1800}"
  local deadline now remaining round

  require_tools
  ensure_dirs
  echo "running" >"$RUNNER_STATE_FILE"
  : >"$RUNNER_LOG_FILE"
  : >"$LOG_DIR/rounds.tsv"
  : >"$LOG_DIR/list.ndjson"
  rm -f "$STOP_FILE" "$FAIL_FILE"
  load_fixtures

  trap 'log "runner interrupted"; echo "stopped" > "$RUNNER_STATE_FILE"; rm -f "$RUNNER_PID_FILE"; exit 0' INT TERM

  deadline="$(( $(ts_epoch) + duration_secs ))"
  log "download-soak-start scenario=$SCENARIO duration_secs=$duration_secs deadline_epoch=$deadline base_url=$BASE_URL"

  if ! wait_ready; then
    echo "failed" >"$RUNNER_STATE_FILE"
    rm -f "$RUNNER_PID_FILE"
    exit 1
  fi

  round=0
  while true; do
    if [[ -f "$STOP_FILE" ]]; then
      log "stop marker detected"
      break
    fi
    now="$(ts_epoch)"
    if (( now >= deadline )); then
      log "deadline reached"
      break
    fi
    remaining="$(( deadline - now ))"
    round="$(( round + 1 ))"
    log "timer remaining_secs=$remaining round=$round"
    run_round "$round" || true
    sleep "$WAIT_BETWEEN"
  done

  if [[ -f "$FAIL_FILE" ]]; then
    log "download-soak-finished rounds=$round result=failed"
    echo "failed" >"$RUNNER_STATE_FILE"
    rm -f "$RUNNER_PID_FILE"
    exit 1
  fi

  log "download-soak-finished rounds=$round result=completed"
  echo "completed" >"$RUNNER_STATE_FILE"
  rm -f "$RUNNER_PID_FILE"
}

start_background() {
  local duration_secs="${1:-1800}"
  ensure_dirs

  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      echo "runner already active pid=$pid scenario=$SCENARIO"
      return 0
    fi
    rm -f "$RUNNER_PID_FILE"
  fi

  nohup bash -lc "cd '$PWD' && SCENARIO='$SCENARIO' BASE_URL='$BASE_URL' TOKEN_FILE='$TOKEN_FILE' RUN_ROOT='$RUN_ROOT_BASE' WAIT_BETWEEN='$WAIT_BETWEEN' READY_TIMEOUT_SECS='$READY_TIMEOUT_SECS' READY_PATH='$READY_PATH' READY_HTTP_CODES='$READY_HTTP_CODES' CONCURRENCY_TARGET='$CONCURRENCY_TARGET' CHURN_MAX_QUEUE='$CHURN_MAX_QUEUE' API_CONNECT_TIMEOUT_SECS='$API_CONNECT_TIMEOUT_SECS' API_MAX_TIME_SECS='$API_MAX_TIME_SECS' DOWNLOAD_FIXTURES_FILE='$DOWNLOAD_FIXTURES_FILE' FIXTURES_ONLY='$FIXTURES_ONLY' '$0' run '$duration_secs'" >"$RUNNER_STDOUT_FILE" 2>&1 &
  echo $! >"$RUNNER_PID_FILE"
  log "runner started pid=$(cat "$RUNNER_PID_FILE") scenario=$SCENARIO duration_secs=$duration_secs"
}

status_runner() {
  ensure_dirs
  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      echo "status=running pid=$pid scenario=$SCENARIO"
    else
      echo "status=stale_pid pid=${pid:-unknown} scenario=$SCENARIO"
    fi
  else
    echo "status=not_running scenario=$SCENARIO"
  fi
  if [[ -f "$RUNNER_STATE_FILE" ]]; then
    echo "runner_state=$(cat "$RUNNER_STATE_FILE")"
  fi
}

stop_runner() {
  ensure_dirs
  touch "$STOP_FILE"
  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      kill "$pid" 2>/dev/null || true
      sleep 1
      if is_pid_alive "$pid"; then
        kill -9 "$pid" 2>/dev/null || true
      fi
    fi
    rm -f "$RUNNER_PID_FILE"
  fi
  local current_state
  current_state="$(cat "$RUNNER_STATE_FILE" 2>/dev/null || true)"
  if [[ "$current_state" != "failed" && "$current_state" != "completed" ]]; then
    echo "stopped" >"$RUNNER_STATE_FILE"
  fi
  log "runner stop requested scenario=$SCENARIO"
}

collect_bundle() {
  ensure_dirs
  local bundle
  bundle="/tmp/rust-mule-download-soak-${SCENARIO}-$(date +%Y%m%d_%H%M%S).tar.gz"
  tar -czf "$bundle" -C "$RUN_ROOT" .
  echo "$bundle"
}

case "${1:-}" in
start)
  start_background "${2:-1800}"
  ;;
run)
  run_foreground "${2:-1800}"
  ;;
status)
  status_runner
  ;;
stop)
  stop_runner
  ;;
collect)
  collect_bundle
  ;;
*)
  echo "usage: SCENARIO=<single_e2e|long_churn|integrity|concurrency> $0 {start [duration_secs]|run [duration_secs]|status|stop|collect}"
  exit 2
  ;;
esac
