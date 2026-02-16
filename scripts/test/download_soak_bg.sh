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
#   READY_TIMEOUT_SECS=180
#   CONCURRENCY_TARGET=20
#   CHURN_MAX_QUEUE=25

SCENARIO="${SCENARIO:-}"
BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"
RUN_ROOT_BASE="${RUN_ROOT:-/tmp/rust-mule-download-soak}"
WAIT_BETWEEN="${WAIT_BETWEEN:-5}"
READY_TIMEOUT_SECS="${READY_TIMEOUT_SECS:-180}"
CONCURRENCY_TARGET="${CONCURRENCY_TARGET:-20}"
CHURN_MAX_QUEUE="${CHURN_MAX_QUEUE:-25}"

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

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
ts_epoch() { date +%s; }
log() { echo "$(ts) $*" | tee -a "$RUNNER_LOG_FILE"; }

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

api_get() {
  local path="$1"
  local auth
  auth="$(auth_header)" || return 1
  curl -sS -H "$auth" "$BASE_URL$path"
}

api_post() {
  local path="$1"
  local json="${2:-{}}"
  local auth
  auth="$(auth_header)" || return 1
  curl -sS -H "$auth" -H "content-type: application/json" -d "$json" "$BASE_URL$path"
}

api_delete() {
  local path="$1"
  local auth
  auth="$(auth_header)" || return 1
  curl -sS -X DELETE -H "$auth" "$BASE_URL$path"
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
  api_post "/api/v1/downloads" "{\"file_name\":\"$name\",\"file_size\":$size,\"file_hash_md4_hex\":\"$md4\"}"
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
    code="$(curl -s -o /dev/null -w '%{http_code}' -H "$auth" "$BASE_URL/api/v1/status" || true)"
    log "ready-check elapsed=${elapsed}s status_code=$code"
    if [[ "$code" == "200" ]]; then
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
    local name size md4 create_json
    name="single-e2e-${round}.bin"
    size="$(( (RANDOM % 20000000) + 1000000 ))"
    md4="$(rand_md4_hex)"
    create_json="$(downloads_create "$name" "$size" "$md4" || true)"
    part="$(echo "$create_json" | jq -r '.download.part_number // empty' 2>/dev/null || true)"
    write_round_row "$round" "create" "part=${part:-none}"
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
  local name size md4

  name="churn-${round}-$(rand_hex16).bin"
  size="$(( (RANDOM % 10000000) + 500000 ))"
  md4="$(rand_md4_hex)"
  downloads_create "$name" "$size" "$md4" >/dev/null || true

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
  local name size md4

  if (( round <= 3 )); then
    name="integrity-${round}.bin"
    size="$(( (RANDOM % 5000000) + 2000000 ))"
    md4="$(rand_md4_hex)"
    downloads_create "$name" "$size" "$md4" >/dev/null || true
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
  local round queue_len i target name size md4 part
  target="$CONCURRENCY_TARGET"
  queue_len="$(downloads_list | jq -r '.queue_len // 0' 2>/dev/null || echo 0)"

  while (( queue_len < target )); do
    name="concurrency-${round}-$(rand_hex16).bin"
    size="$(( (RANDOM % 15000000) + 500000 ))"
    md4="$(rand_md4_hex)"
    downloads_create "$name" "$size" "$md4" >/dev/null || true
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

  nohup bash -lc "cd '$PWD' && SCENARIO='$SCENARIO' BASE_URL='$BASE_URL' TOKEN_FILE='$TOKEN_FILE' RUN_ROOT='$RUN_ROOT_BASE' WAIT_BETWEEN='$WAIT_BETWEEN' READY_TIMEOUT_SECS='$READY_TIMEOUT_SECS' CONCURRENCY_TARGET='$CONCURRENCY_TARGET' CHURN_MAX_QUEUE='$CHURN_MAX_QUEUE' '$0' run '$duration_secs'" >"$RUNNER_STDOUT_FILE" 2>&1 &
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
