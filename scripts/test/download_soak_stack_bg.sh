#!/usr/bin/env bash
set -euo pipefail

# End-to-end background runner:
# - builds latest rust-mule
# - stages isolated run dir under /tmp/rustmule-run-<timestamp>
# - configures config.toml for soak use
# - starts rust-mule
# - waits for health/token
# - runs download_soak_band.sh
#
# Commands:
#   start
#   run          # internal foreground runner
#   status
#   stop
#   collect
#
# Environment overrides:
#   ROOT=/path/to/rust-mule-repo
#   STACK_ROOT=/tmp/rust-mule-download-stack
#   RUN_DIR=/tmp/rustmule-run-<timestamp>
#   API_PORT=17835
#   SAM_HOST=10.99.0.2
#   SAM_PORT=7656
#   LOG_LEVEL=info
#   BUILD_PROFILE=release
#   BUILD_CMD="cargo build --release"
#   HEALTH_TIMEOUT_SECS=300
#   POLL_SECS=2
#
# Forwarded to download_soak_band.sh:
#   INTEGRITY_SECS, SINGLE_E2E_SECS, CONCURRENCY_SECS, LONG_CHURN_SECS,
#   CONCURRENCY_TARGET, CHURN_MAX_QUEUE, DOWNLOAD_FIXTURES_FILE, FIXTURES_ONLY

ROOT="${ROOT:-$PWD}"
STACK_ROOT="${STACK_ROOT:-/tmp/rust-mule-download-stack}"
RUN_DIR="${RUN_DIR:-/tmp/rustmule-run-$(date +%Y%m%d_%H%M%S)}"

API_PORT="${API_PORT:-17835}"
SAM_HOST="${SAM_HOST:-10.99.0.2}"
SAM_PORT="${SAM_PORT:-7656}"
LOG_LEVEL="${LOG_LEVEL:-info}"
BUILD_PROFILE="${BUILD_PROFILE:-release}"
BUILD_CMD="${BUILD_CMD:-cargo build --release}"
HEALTH_TIMEOUT_SECS="${HEALTH_TIMEOUT_SECS:-300}"
POLL_SECS="${POLL_SECS:-2}"

BASE_URL="http://127.0.0.1:${API_PORT}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SELF_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
CONTROL_DIR="$STACK_ROOT/control"
LOG_DIR="$STACK_ROOT/logs"

RUNNER_PID_FILE="$CONTROL_DIR/runner.pid"
RUN_DIR_FILE="$CONTROL_DIR/run_dir"
RUNNER_STATE_FILE="$CONTROL_DIR/runner.state"
APP_PID_FILE="$CONTROL_DIR/app.pid"
STOP_FILE="$CONTROL_DIR/stop.requested"
RUNNER_LOG_FILE="$LOG_DIR/stack.log"
RUNNER_STDOUT_FILE="$LOG_DIR/stack.out"

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
log() { echo "$(ts) $*" | tee -a "$RUNNER_LOG_FILE"; }

ensure_toolchain_path() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi
  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
}

ensure_dirs() {
  mkdir -p "$CONTROL_DIR" "$LOG_DIR"
}

is_pid_alive() {
  local pid="$1"
  kill -0 "$pid" 2>/dev/null
}

read_run_dir() {
  cat "$RUN_DIR_FILE" 2>/dev/null || true
}

binary_path() {
  echo "$ROOT/target/$BUILD_PROFILE/rust-mule"
}

stop_pid_if_alive() {
  local pid="$1"
  [[ -n "$pid" ]] || return 0
  if is_pid_alive "$pid"; then
    kill "$pid" 2>/dev/null || true
    sleep 1
    if is_pid_alive "$pid"; then
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
}

stop_process_group_if_alive() {
  local pid="$1"
  [[ -n "$pid" ]] || return 0
  if ! is_pid_alive "$pid"; then
    return 0
  fi
  local pgid
  pgid="$(ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ' || true)"
  if [[ -z "$pgid" ]]; then
    stop_pid_if_alive "$pid"
    return 0
  fi

  kill -TERM "-$pgid" 2>/dev/null || true
  sleep 1
  if is_pid_alive "$pid"; then
    kill -KILL "-$pgid" 2>/dev/null || true
  fi
}

stop_run_dir_processes() {
  local run_dir="$1"
  [[ -n "$run_dir" && -d "$run_dir" ]] || return 0

  local proc pid cwd cmdline
  for proc in /proc/[0-9]*; do
    pid="${proc##*/}"
    [[ -d "$proc" ]] || continue
    cwd="$(readlink -f "$proc/cwd" 2>/dev/null || true)"
    cmdline="$(tr '\0' ' ' <"$proc/cmdline" 2>/dev/null || true)"
    if [[ "$cwd" == "$run_dir"* || "$cmdline" == *"$run_dir"* ]]; then
      stop_pid_if_alive "$pid"
    fi
  done
}

stop_download_soak_runners() {
  local run_dir="$1"
  [[ -n "$run_dir" && -d "$run_dir" ]] || return 0

  local token_file
  token_file="$run_dir/data/api.token"
  BASE_URL="$BASE_URL" TOKEN_FILE="$token_file" "$SCRIPT_DIR/download_soak_integrity_bg.sh" stop >/dev/null 2>&1 || true
  BASE_URL="$BASE_URL" TOKEN_FILE="$token_file" "$SCRIPT_DIR/download_soak_single_e2e_bg.sh" stop >/dev/null 2>&1 || true
  BASE_URL="$BASE_URL" TOKEN_FILE="$token_file" "$SCRIPT_DIR/download_soak_concurrency_bg.sh" stop >/dev/null 2>&1 || true
  BASE_URL="$BASE_URL" TOKEN_FILE="$token_file" "$SCRIPT_DIR/download_soak_long_churn_bg.sh" stop >/dev/null 2>&1 || true
}

cleanup_children() {
  local app_pid
  app_pid="$(cat "$APP_PID_FILE" 2>/dev/null || true)"
  stop_pid_if_alive "$app_pid"
  rm -f "$APP_PID_FILE"
}

wait_for_health() {
  local start now elapsed code token_file run_dir token auth downloads_code
  run_dir="$(read_run_dir)"
  token_file="$run_dir/data/api.token"
  start="$(date +%s)"

  while true; do
    now="$(date +%s)"
    elapsed="$((now - start))"
    if (( elapsed > HEALTH_TIMEOUT_SECS )); then
      log "ERROR: health wait timeout after ${HEALTH_TIMEOUT_SECS}s"
      return 1
    fi
    code="$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/v1/health" || true)"
    if [[ "$code" == "200" && -s "$token_file" ]]; then
      token="$(tr -d '\r\n' <"$token_file" 2>/dev/null || true)"
      if [[ -n "$token" ]]; then
        auth="Authorization: Bearer $token"
        downloads_code="$(curl -s -o /dev/null -w '%{http_code}' -H "$auth" "$BASE_URL/api/v1/downloads" || true)"
        if [[ "$downloads_code" == "200" ]]; then
          log "health-check-ok code=$code downloads_code=$downloads_code token_file=$token_file elapsed=${elapsed}s"
          return 0
        fi
        log "health-check-wait code=$code downloads_code=$downloads_code token_file=$token_file elapsed=${elapsed}s"
      else
        log "health-check-wait code=$code token_file=$token_file token=empty elapsed=${elapsed}s"
      fi
    fi
    sleep "$POLL_SECS"
  done
}

configure_run_config() {
  local run_dir="$1"
  local run_tag
  run_tag="$(basename "$run_dir")"

  cp "$ROOT/config.toml" "$run_dir/config.toml"
  awk \
    -v sam_host="$SAM_HOST" \
    -v sam_port="$SAM_PORT" \
    -v api_port="$API_PORT" \
    -v log_level="$LOG_LEVEL" \
    -v session_name="rust-mule-stack-$run_tag" \
    '
    BEGIN { section = "" }
    /^\[.*\]$/ { section = $0; print; next }
    section == "[sam]" && $0 ~ /^host = / { print "host = \"" sam_host "\""; next }
    section == "[sam]" && $0 ~ /^port = / { print "port = " sam_port; next }
    section == "[sam]" && $0 ~ /^session_name = / { print "session_name = \"" session_name "\""; next }
    section == "[general]" && $0 ~ /^log_level = / { print "log_level = \"" log_level "\""; next }
    section == "[general]" && $0 ~ /^auto_open_ui = / { print "auto_open_ui = false"; next }
    section == "[api]" && $0 ~ /^port = / { print "port = " api_port; next }
    { print }
  ' "$run_dir/config.toml" >"$run_dir/config.toml.tmp"
  mv "$run_dir/config.toml.tmp" "$run_dir/config.toml"
}

start_app() {
  local run_dir="$1"
  mkdir -p "$run_dir/data" "$run_dir/data/logs" "$run_dir/data/download" "$run_dir/data/incoming"
  cp "$(binary_path)" "$run_dir/rust-mule"
  chmod +x "$run_dir/rust-mule"

  (cd "$run_dir" && nohup ./rust-mule >"$run_dir/rust-mule.out" 2>&1 & echo $! >"$APP_PID_FILE")
  log "app-started run_dir=$run_dir app_pid=$(cat "$APP_PID_FILE") base_url=$BASE_URL"
}

run_foreground() {
  local staged_script_dir
  ensure_dirs
  ensure_toolchain_path
  : >"$RUNNER_LOG_FILE"
  echo "running" >"$RUNNER_STATE_FILE"
  rm -f "$STOP_FILE"

  trap 'log "runner interrupted"; echo "stopped" >"$RUNNER_STATE_FILE"; cleanup_children; rm -f "$RUNNER_PID_FILE"; exit 0' INT TERM

  log "build-start cmd=$BUILD_CMD"
  # Run build in current shell context so PATH/toolchain bootstrap is preserved.
  if ! (cd "$ROOT" && eval "$BUILD_CMD"); then
    log "ERROR: build failed cmd=$BUILD_CMD"
    echo "failed" >"$RUNNER_STATE_FILE"
    cleanup_children
    rm -f "$RUNNER_PID_FILE"
    exit 1
  fi
  if [[ ! -x "$(binary_path)" ]]; then
    log "ERROR: built binary not found at $(binary_path)"
    echo "failed" >"$RUNNER_STATE_FILE"
    rm -f "$RUNNER_PID_FILE"
    exit 1
  fi
  log "build-ok binary=$(binary_path)"

  mkdir -p "$RUN_DIR"
  echo "$RUN_DIR" >"$RUN_DIR_FILE"
  log "run-dir=$RUN_DIR"

  configure_run_config "$RUN_DIR"
  start_app "$RUN_DIR"

  if ! wait_for_health; then
    echo "failed" >"$RUNNER_STATE_FILE"
    cleanup_children
    rm -f "$RUNNER_PID_FILE"
    exit 1
  fi

  local band_out_dir token_file band_exit=0
  token_file="$RUN_DIR/data/api.token"
  band_out_dir="$RUN_DIR/soak-band"
  staged_script_dir="$RUN_DIR/soak-scripts"
  mkdir -p "$band_out_dir"
  rm -rf "$staged_script_dir"
  mkdir -p "$staged_script_dir"
  cp -f "$SCRIPT_DIR"/download_soak_* "$staged_script_dir"/
  chmod +x "$staged_script_dir"/download_soak_*
  log "band-start out_dir=$band_out_dir token_file=$token_file"
  log "band-scripts-staged dir=$staged_script_dir"

  BASE_URL="$BASE_URL" \
    TOKEN_FILE="$token_file" \
    OUT_DIR="$band_out_dir" \
    INTEGRITY_SECS="${INTEGRITY_SECS:-3600}" \
    SINGLE_E2E_SECS="${SINGLE_E2E_SECS:-3600}" \
    CONCURRENCY_SECS="${CONCURRENCY_SECS:-7200}" \
    LONG_CHURN_SECS="${LONG_CHURN_SECS:-7200}" \
    CONCURRENCY_TARGET="${CONCURRENCY_TARGET:-20}" \
    CHURN_MAX_QUEUE="${CHURN_MAX_QUEUE:-25}" \
    DOWNLOAD_FIXTURES_FILE="${DOWNLOAD_FIXTURES_FILE:-}" \
    FIXTURES_ONLY="${FIXTURES_ONLY:-0}" \
    "$staged_script_dir/download_soak_band.sh" || band_exit=$?

  if (( band_exit != 0 )); then
    log "ERROR: band-run failed exit=$band_exit"
    echo "failed" >"$RUNNER_STATE_FILE"
    cleanup_children
    rm -f "$RUNNER_PID_FILE"
    exit "$band_exit"
  fi

  echo "completed" >"$RUNNER_STATE_FILE"
  log "stack-completed run_dir=$RUN_DIR"
  cleanup_children
  rm -f "$RUNNER_PID_FILE"
}

start_background() {
  ensure_dirs
  ensure_toolchain_path
  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      echo "runner already active pid=$pid"
      return 0
    fi
    rm -f "$RUNNER_PID_FILE"
  fi

  nohup bash -lc "cd '$ROOT' && ROOT='$ROOT' STACK_ROOT='$STACK_ROOT' RUN_DIR='$RUN_DIR' API_PORT='$API_PORT' SAM_HOST='$SAM_HOST' SAM_PORT='$SAM_PORT' LOG_LEVEL='$LOG_LEVEL' BUILD_PROFILE='$BUILD_PROFILE' BUILD_CMD='$BUILD_CMD' HEALTH_TIMEOUT_SECS='$HEALTH_TIMEOUT_SECS' POLL_SECS='$POLL_SECS' INTEGRITY_SECS='${INTEGRITY_SECS:-3600}' SINGLE_E2E_SECS='${SINGLE_E2E_SECS:-3600}' CONCURRENCY_SECS='${CONCURRENCY_SECS:-7200}' LONG_CHURN_SECS='${LONG_CHURN_SECS:-7200}' CONCURRENCY_TARGET='${CONCURRENCY_TARGET:-20}' CHURN_MAX_QUEUE='${CHURN_MAX_QUEUE:-25}' DOWNLOAD_FIXTURES_FILE='${DOWNLOAD_FIXTURES_FILE:-}' FIXTURES_ONLY='${FIXTURES_ONLY:-0}' '$SELF_PATH' run" >"$RUNNER_STDOUT_FILE" 2>&1 &
  echo $! >"$RUNNER_PID_FILE"
  echo "$RUN_DIR" >"$RUN_DIR_FILE"
  local pid
  pid="$(cat "$RUNNER_PID_FILE")"
  sleep 1
  if ! is_pid_alive "$pid"; then
    echo "failed" >"$RUNNER_STATE_FILE"
    log "ERROR: stack runner exited immediately pid=$pid (see $RUNNER_STDOUT_FILE)"
    rm -f "$RUNNER_PID_FILE"
    return 1
  fi
  log "stack-runner-started pid=$pid run_dir=$RUN_DIR"
}

status_runner() {
  ensure_dirs
  local run_dir pid app_pid
  run_dir="$(read_run_dir)"

  if [[ -f "$RUNNER_PID_FILE" ]]; then
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      echo "status=running pid=$pid"
    else
      echo "status=stale_pid pid=${pid:-unknown}"
    fi
  else
    echo "status=not_running"
  fi
  [[ -f "$RUNNER_STATE_FILE" ]] && echo "runner_state=$(cat "$RUNNER_STATE_FILE")"
  [[ -n "$run_dir" ]] && echo "run_dir=$run_dir"
  [[ -f "$APP_PID_FILE" ]] && app_pid="$(cat "$APP_PID_FILE")" || app_pid=""
  if [[ -n "$app_pid" ]]; then
    if is_pid_alive "$app_pid"; then
      echo "app_pid=$app_pid alive=1"
    else
      echo "app_pid=$app_pid alive=0"
    fi
  fi
}

stop_runner() {
  ensure_dirs
  touch "$STOP_FILE"
  local run_dir
  run_dir="$(read_run_dir)"

  stop_download_soak_runners "$run_dir"

  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    stop_process_group_if_alive "$pid"
    rm -f "$RUNNER_PID_FILE"
  fi

  cleanup_children
  stop_run_dir_processes "$run_dir"
  local current_state
  current_state="$(cat "$RUNNER_STATE_FILE" 2>/dev/null || true)"
  if [[ "$current_state" != "failed" && "$current_state" != "completed" ]]; then
    echo "stopped" >"$RUNNER_STATE_FILE"
  fi
  log "stack-stop-requested"
}

collect_bundle() {
  ensure_dirs
  local run_dir bundle
  run_dir="$(read_run_dir)"
  if [[ -z "$run_dir" || ! -d "$run_dir" ]]; then
    echo "ERROR: no run_dir available to collect" >&2
    exit 1
  fi
  bundle="/tmp/rust-mule-download-stack-$(date +%Y%m%d_%H%M%S).tar.gz"
  tar -czf "$bundle" -C "$(dirname "$run_dir")" "$(basename "$run_dir")"
  echo "$bundle"
}

case "${1:-}" in
start)
  start_background
  ;;
run)
  run_foreground
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
  echo "usage: $SELF_PATH {start|run|status|stop|collect}"
  exit 2
  ;;
esac
