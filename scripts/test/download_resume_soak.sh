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
#   API_PORT=17835
#   RESUME_SCENARIO=concurrency
#   WAIT_TIMEOUT_SECS=21600
#   HEALTH_TIMEOUT_SECS=300
#   POLL_SECS=2
#   RESUME_OUT_DIR=/tmp/rust-mule-download-resume-<timestamp>

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STACK_SCRIPT="${STACK_SCRIPT:-$SCRIPT_DIR/download_soak_stack_bg.sh}"
STACK_ROOT="${STACK_ROOT:-/tmp/rust-mule-download-stack}"
CONTROL_DIR="$STACK_ROOT/control"

API_PORT="${API_PORT:-17835}"
BASE_URL="${BASE_URL:-http://127.0.0.1:${API_PORT}}"
RESUME_SCENARIO="${RESUME_SCENARIO:-concurrency}"

WAIT_TIMEOUT_SECS="${WAIT_TIMEOUT_SECS:-21600}"
HEALTH_TIMEOUT_SECS="${HEALTH_TIMEOUT_SECS:-300}"
POLL_SECS="${POLL_SECS:-2}"
RESUME_OUT_DIR="${RESUME_OUT_DIR:-/tmp/rust-mule-download-resume-$(date +%Y%m%d_%H%M%S)}"

RUN_DIR=""
STATUS_TSV=""
STACK_TARBALL=""
CRASH_EPOCH=0
RESTART_EPOCH=0

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
ts_epoch() { date +%s; }
log() { echo "$(ts) $*"; }

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
  local start now elapsed line status state
  start="$(ts_epoch)"
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
      log "scenario-check scenario=$scenario status=${status:-unknown} state=${state:-unknown}"
      if [[ "$status" == "running" && "$state" == "running" ]]; then
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
  ps -eo pid,args \
    | awk -v rd="$RUN_DIR" '$0 ~ (rd "/rust-mule") { c++ } END { print c + 0 }'
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
      ps -eo pid,args | awk -v rd="$RUN_DIR" '$0 ~ (rd "/rust-mule") { print }' >&2 || true
      exit 1
    fi
    sleep "$POLL_SECS"
  done
}

api_token() {
  cat "$RUN_DIR/data/api.token"
}

snapshot_downloads() {
  local label="$1"
  local token
  token="$(api_token)"
  curl -sS -H "Authorization: Bearer $token" "$BASE_URL/api/v1/downloads" >"$RESUME_OUT_DIR/${label}_downloads.json"

  {
    echo "timestamp=$(ts)"
    echo "run_dir=$RUN_DIR"
    echo "label=$label"
    echo "downloads_count=$(jq -r '.downloads | length' "$RESUME_OUT_DIR/${label}_downloads.json")"
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

crash_app() {
  local app_pid
  app_pid="$(cat "$CONTROL_DIR/app.pid" 2>/dev/null || true)"
  [[ -n "$app_pid" ]] || {
    echo "ERROR: missing app pid in $CONTROL_DIR/app.pid" >&2
    exit 1
  }
  kill -9 "$app_pid"
  CRASH_EPOCH="$(ts_epoch)"
  echo "$app_pid" >"$RESUME_OUT_DIR/crashed_app.pid"
  log "crashed app pid=$app_pid"

  wait_for_app_exit_by_pid "$app_pid" "$HEALTH_TIMEOUT_SECS"
  wait_for_run_dir_processes_gone "$HEALTH_TIMEOUT_SECS"

  # Health may remain up if some unrelated process already serves the port.
  # We no longer require code=000; process-level stop verification is authoritative.
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/v1/health" || true)"
  log "post-crash health status_code=$code (informational)"
}

restart_app() {
  local new_pid
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
  local start now elapsed baseline_lines current_lines line status state

  baseline_lines="$(wc -l <"$STATUS_TSV" 2>/dev/null || echo 0)"
  start="$(ts_epoch)"
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
      log "post-restart scenario=$scenario status=${status:-unknown} state=${state:-unknown} lines=$current_lines"
      if (( current_lines > baseline_lines + 2 )) && [[ "$status" == "running" || "$state" == "completed" ]]; then
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
  local start now elapsed out runner_state
  start="$(ts_epoch)"
  while true; do
    now="$(ts_epoch)"
    elapsed="$((now - start))"
    if (( elapsed > WAIT_TIMEOUT_SECS )); then
      echo "ERROR: timed out waiting for stack terminal state" >&2
      return 1
    fi
    out="$(stack_status_raw)"
    runner_state="$(status_field "$out" "runner_state")"
    log "stack-state runner_state=${runner_state:-unknown}"
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

  out="$(stack_status_raw)"
  status="$(status_field "$out" "status")"
  if [[ "$status" == "running" ]]; then
    echo "ERROR: stack runner already running; stop it first or use separate STACK_ROOT." >&2
    exit 1
  fi

  log "starting stack soak via $STACK_SCRIPT"
  "$STACK_SCRIPT" start

  wait_for_run_dir
  wait_for_status_file
  log "run_dir=$RUN_DIR status_tsv=$STATUS_TSV"

  wait_for_scenario_running "$RESUME_SCENARIO" "$WAIT_TIMEOUT_SECS"
  snapshot_downloads "pre"

  crash_app

  restart_app
  wait_for_health_code "200" "$HEALTH_TIMEOUT_SECS"
  snapshot_downloads "post"

  wait_for_post_restart_progress "$RESUME_SCENARIO" "$HEALTH_TIMEOUT_SECS"

  if ! wait_for_stack_terminal; then
    log "stack terminal state is non-completed; requesting stop"
    "$STACK_SCRIPT" stop >/dev/null 2>&1 || true
  fi

  collect_stack_bundle
  write_report
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
