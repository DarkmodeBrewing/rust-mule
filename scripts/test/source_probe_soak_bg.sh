#!/usr/bin/env bash
set -euo pipefail

# Timed, background-safe two-instance source publish/search soak harness.
#
# Commands:
#   start [duration_secs]
#   run [duration_secs]            # internal foreground runner
#   status
#   stop
#   collect
#
# Environment overrides:
#   ROOT=/path/to/rust-mule
#   A_SRC=/path/to/mule-a
#   B_SRC=/path/to/mule-b
#   RUN_ROOT=/tmp/rust-mule-soak-bg
#   A_URL=http://127.0.0.1:17835
#   B_URL=http://127.0.0.1:17836
#   WAIT_PUBLISH=20
#   WAIT_SEARCH=20
#   WAIT_BETWEEN=5
#   MISS_RECHECK_ATTEMPTS=1
#   MISS_RECHECK_DELAY=20
#   SOAK_FRESH_IDENTITY=1
#   SOAK_RUN_TAG=custom-tag
#   READY_TIMEOUT_SECS=1200

ROOT="${ROOT:-$PWD}"
A_SRC="${A_SRC:-$ROOT/../../mule-a}"
B_SRC="${B_SRC:-$ROOT/../../mule-b}"
RUN_ROOT="${RUN_ROOT:-/tmp/rust-mule-soak-bg}"
LOG_DIR="$RUN_ROOT/logs"
A_DIR="$RUN_ROOT/mule-a"
B_DIR="$RUN_ROOT/mule-b"

A_URL="${A_URL:-http://127.0.0.1:17835}"
B_URL="${B_URL:-http://127.0.0.1:17836}"

WAIT_PUBLISH="${WAIT_PUBLISH:-20}"
WAIT_SEARCH="${WAIT_SEARCH:-20}"
WAIT_BETWEEN="${WAIT_BETWEEN:-5}"
MISS_RECHECK_ATTEMPTS="${MISS_RECHECK_ATTEMPTS:-1}"
MISS_RECHECK_DELAY="${MISS_RECHECK_DELAY:-20}"
SOAK_FRESH_IDENTITY="${SOAK_FRESH_IDENTITY:-1}"
READY_TIMEOUT_SECS="${READY_TIMEOUT_SECS:-1200}"
RUN_TAG="${SOAK_RUN_TAG:-}"
SESSION_A=""
SESSION_B=""

RUNNER_PID_FILE="$RUN_ROOT/runner.pid"
RUNNER_LOG_FILE="$LOG_DIR/runner.log"
RUNNER_STDOUT_FILE="$LOG_DIR/runner.out"
RUNNER_STATE_FILE="$RUN_ROOT/runner.state"
STOP_FILE="$RUN_ROOT/stop.requested"

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
ts_epoch() { date +%s; }
rand_hex16() { hexdump -n 16 -e '16/1 "%02x"' /dev/urandom; }
log() { echo "$(ts) $*" | tee -a "$RUNNER_LOG_FILE"; }

response_has_sources() {
  local payload="${1:-}"
  echo "$payload" | grep -Eq '"sources"[[:space:]]*:[[:space:]]*\[[[:space:]]*\{'
}

url_port() {
  local url="$1"
  local hostport
  hostport="$(echo "$url" | sed -E 's#^[a-zA-Z]+://([^/]+)/?.*$#\1#')"
  echo "$hostport" | awk -F: '{print $NF}'
}

ensure_dirs() {
  mkdir -p "$RUN_ROOT" "$LOG_DIR"
}

init_run_identity() {
  if [[ -z "$RUN_TAG" ]]; then
    RUN_TAG="$(date +%Y%m%d%H%M%S)-$$"
  fi
  SESSION_A="rust-mule-a-soak-${RUN_TAG}"
  SESSION_B="rust-mule-b-soak-${RUN_TAG}"
}

is_pid_alive() {
  local pid="$1"
  kill -0 "$pid" 2>/dev/null
}

kill_pid_gracefully() {
  local pid="$1"
  local label="$2"
  [[ -n "$pid" ]] || return 0
  [[ -d "/proc/$pid" ]] || return 0

  kill "$pid" 2>/dev/null || true
  sleep 1
  if kill -0 "$pid" 2>/dev/null; then
    kill -9 "$pid" 2>/dev/null || true
  fi
  if kill -0 "$pid" 2>/dev/null; then
    log "WARN: failed to kill $label pid=$pid"
    return 1
  fi
  log "killed $label pid=$pid"
}

is_soak_node_pid() {
  local pid="$1"
  [[ -n "$pid" ]] || return 1
  [[ -d "/proc/$pid" ]] || return 1

  local cmdline cwd root_abs
  cmdline="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)"
  cwd="$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)"
  root_abs="$(readlink -f "$RUN_ROOT" 2>/dev/null || echo "$RUN_ROOT")"

  if [[ "$cwd" == "$root_abs"* ]]; then
    return 0
  fi
  if [[ "$cmdline" == *"$A_DIR"* || "$cmdline" == *"$B_DIR"* || "$cmdline" == *"$root_abs"* ]]; then
    return 0
  fi
  return 1
}

configure_b_instance() {
  sed -i -E "s#^session_name = .*#session_name = \"$SESSION_B\"#" "$B_DIR/config.toml" || true
  sed -i 's/forward_port = 40000/forward_port = 40001/' "$B_DIR/config.toml" || true
  sed -i 's/udp_port = 4665/udp_port = 4666/' "$B_DIR/config.toml" || true
  sed -i "s/port = 17835/port = $(url_port "$B_URL")/" "$B_DIR/config.toml" || true
}

configure_a_instance() {
  sed -i -E "s#^session_name = .*#session_name = \"$SESSION_A\"#" "$A_DIR/config.toml" || true
  sed -i "s/port = 17835/port = $(url_port "$A_URL")/" "$A_DIR/config.toml" || true
}

port_is_busy() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    if ss -ltnH | awk '{print $4}' | grep -Eq "(^|:|\\])$port$"; then
      return 0
    fi
  fi
  if command -v lsof >/dev/null 2>&1; then
    if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      return 0
    fi
  fi
  if command -v nc >/dev/null 2>&1; then
    if nc -z -w1 127.0.0.1 "$port" >/dev/null 2>&1; then
      return 0
    fi
    if nc -z -w1 ::1 "$port" >/dev/null 2>&1; then
      return 0
    fi
  fi
  if command -v timeout >/dev/null 2>&1; then
    if timeout 1 bash -lc ":</dev/tcp/127.0.0.1/$port" >/dev/null 2>&1; then
      return 0
    fi
  fi
  return 1
}

ensure_ports_available() {
  local a_port b_port
  a_port="$(url_port "$A_URL")"
  b_port="$(url_port "$B_URL")"

  if port_is_busy "$a_port"; then
    log "ERROR: A_URL port $a_port is already in use. Stop existing process or pick another A_URL."
    return 1
  fi
  if port_is_busy "$b_port"; then
    log "ERROR: B_URL port $b_port is already in use. Stop existing process or pick another B_URL."
    return 1
  fi
}

verify_node_pid_context() {
  local pid="$1"
  local expected_dir="$2"
  local cwd

  if [[ ! -d "/proc/$pid" ]]; then
    return 1
  fi
  cwd="$(readlink -f "/proc/$pid/cwd" 2>/dev/null || true)"
  [[ "$cwd" == "$(readlink -f "$expected_dir")" ]]
}

port_listener_pids() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -nP -t -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | sort -u
    return 0
  fi
  if command -v ss >/dev/null 2>&1; then
    ss -ltnp 2>/dev/null \
      | awk -v p=":$port" '$4 ~ p {print $NF}' \
      | sed -nE 's/.*pid=([0-9]+).*/\1/p' \
      | sort -u
    return 0
  fi
  return 1
}

kill_pid_if_rust_mule() {
  local pid="$1"
  [[ -n "$pid" ]] || return 0
  [[ -d "/proc/$pid" ]] || return 0

  if is_soak_node_pid "$pid"; then
    kill_pid_gracefully "$pid" "soak listener"
  fi
}

stop_port_listeners() {
  local a_port b_port pids pid
  a_port="$(url_port "$A_URL")"
  b_port="$(url_port "$B_URL")"
  pids="$( { port_listener_pids "$a_port"; port_listener_pids "$b_port"; } 2>/dev/null | sort -u )"
  for pid in $pids; do
    kill_pid_if_rust_mule "$pid"
  done
}

start_nodes() {
  ensure_ports_available
  init_run_identity

  rm -rf "$A_DIR" "$B_DIR"
  cp -a "$A_SRC" "$A_DIR"
  cp -a "$B_SRC" "$B_DIR"

  configure_a_instance
  configure_b_instance

  rm -f "$A_DIR/data/rust-mule.lock" "$B_DIR/data/rust-mule.lock"
  if [[ "$SOAK_FRESH_IDENTITY" == "1" ]]; then
    rm -f "$A_DIR/data/sam.keys" "$B_DIR/data/sam.keys"
  fi
  mkdir -p "$A_DIR/data/logs" "$B_DIR/data/logs"

  log "run-tag=$RUN_TAG session_a=$SESSION_A session_b=$SESSION_B fresh_identity=$SOAK_FRESH_IDENTITY"

  (cd "$A_DIR" && nohup ./rust-mule >"$LOG_DIR/a.out" 2>&1 & echo $! >"$LOG_DIR/a.pid")
  (cd "$B_DIR" && nohup ./rust-mule >"$LOG_DIR/b.out" 2>&1 & echo $! >"$LOG_DIR/b.pid")

  if ! verify_node_pid_context "$(cat "$LOG_DIR/a.pid")" "$A_DIR"; then
    log "ERROR: A pid is not running from expected directory $A_DIR"
    return 1
  fi
  if ! verify_node_pid_context "$(cat "$LOG_DIR/b.pid")" "$B_DIR"; then
    log "ERROR: B pid is not running from expected directory $B_DIR"
    return 1
  fi

  log "started A pid=$(cat "$LOG_DIR/a.pid") B pid=$(cat "$LOG_DIR/b.pid")"
}

stop_nodes() {
  local a_pid b_pid
  a_pid="$(cat "$LOG_DIR/a.pid" 2>/dev/null || true)"
  b_pid="$(cat "$LOG_DIR/b.pid" 2>/dev/null || true)"

  if [[ -f "$LOG_DIR/a.pid" ]]; then
    kill_pid_gracefully "$a_pid" "node A"
  fi
  if [[ -f "$LOG_DIR/b.pid" ]]; then
    kill_pid_gracefully "$b_pid" "node B"
  fi
  rm -f "$LOG_DIR/a.pid" "$LOG_DIR/b.pid"
  log "node stop requested"
}

stop_run_root_nodes() {
  if [[ ! -d /proc ]]; then
    return 0
  fi

  local pid
  for proc_dir in /proc/[0-9]*; do
    pid="${proc_dir##*/}"
    if is_soak_node_pid "$pid"; then
      kill_pid_gracefully "$pid" "soak process"
    fi
  done
}

wait_ready() {
  local start now elapsed ta tb a_code b_code consecutive_forbidden
  start="$(ts_epoch)"
  consecutive_forbidden=0
  while true; do
    now="$(ts_epoch)"
    elapsed="$(( now - start ))"
    if (( elapsed > READY_TIMEOUT_SECS )); then
      log "ERROR: readiness timeout after ${READY_TIMEOUT_SECS}s"
      return 1
    fi

    ta="$(cat "$A_DIR/data/api.token" 2>/dev/null || true)"
    tb="$(cat "$B_DIR/data/api.token" 2>/dev/null || true)"
    a_code="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $ta" "$A_URL/api/v1/status" || true)"
    b_code="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $tb" "$B_URL/api/v1/status" || true)"
    log "ready-check elapsed=${elapsed}s A=$a_code B=$b_code"

    if [[ "$a_code" == "200" && "$b_code" == "200" ]]; then
      return 0
    fi

    if [[ "$a_code" == "403" && "$b_code" == "403" ]]; then
      consecutive_forbidden="$((consecutive_forbidden + 1))"
      if (( consecutive_forbidden >= 6 )); then
        log "ERROR: readiness got repeated 403s; likely wrong process on ports or token mismatch"
        return 1
      fi
    else
      consecutive_forbidden=0
    fi
    sleep 5
  done
}

run_round() {
  local round="$1"
  local file_id size ta tb a_pub b_srch b_get b_get_final a_st b_st recheck_idx

  ta="$(cat "$A_DIR/data/api.token")"
  tb="$(cat "$B_DIR/data/api.token")"
  file_id="$(rand_hex16)"
  size="$(( (RANDOM % 5000000) + 1024 ))"

  log "round=$round file=$file_id size=$size"

  a_pub="$(curl -sS -H "Authorization: Bearer $ta" -H 'content-type: application/json' \
    -d "{\"file_id_hex\":\"$file_id\",\"file_size\":$size}" \
    "$A_URL/api/v1/kad/publish_source" || true)"

  sleep "$WAIT_PUBLISH"

  b_srch="$(curl -sS -H "Authorization: Bearer $tb" -H 'content-type: application/json' \
    -d "{\"file_id_hex\":\"$file_id\",\"file_size\":$size}" \
    "$B_URL/api/v1/kad/search_sources" || true)"

  sleep "$WAIT_SEARCH"

  b_get="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/kad/sources/$file_id" || true)"
  b_get_final="$b_get"

  if ! response_has_sources "$b_get_final"; then
    recheck_idx=1
    while (( recheck_idx <= MISS_RECHECK_ATTEMPTS )); do
      log "round=$round miss_recheck attempt=$recheck_idx delay_secs=$MISS_RECHECK_DELAY file=$file_id"
      sleep "$MISS_RECHECK_DELAY"
      b_get_final="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/kad/sources/$file_id" || true)"
      if response_has_sources "$b_get_final"; then
        log "round=$round miss_recheck_hit attempt=$recheck_idx file=$file_id"
        break
      fi
      recheck_idx="$(( recheck_idx + 1 ))"
    done
  fi

  a_st="$(curl -sS -H "Authorization: Bearer $ta" "$A_URL/api/v1/status" || true)"
  b_st="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/status" || true)"

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$(ts)" "$round" "$file_id" "$a_pub" "$b_srch" "$b_get_final" >> "$LOG_DIR/rounds.tsv"
  printf '%s\tA\t%s\n' "$(ts)" "$a_st" >> "$LOG_DIR/status.ndjson"
  printf '%s\tB\t%s\n' "$(ts)" "$b_st" >> "$LOG_DIR/status.ndjson"
}

cleanup_runner() {
  stop_nodes || true
  rm -f "$RUNNER_PID_FILE"
}

run_foreground() {
  local duration_secs="${1:-1800}"
  local deadline now remaining round

  ensure_dirs
  echo "running" >"$RUNNER_STATE_FILE"
  : >"$RUNNER_LOG_FILE"
  : >"$LOG_DIR/rounds.tsv"
  : >"$LOG_DIR/status.ndjson"
  rm -f "$STOP_FILE"

  trap 'log "runner interrupted"; echo "stopped" > "$RUNNER_STATE_FILE"; cleanup_runner; exit 0' INT TERM

  if [[ ! -x "$A_SRC/rust-mule" || ! -x "$B_SRC/rust-mule" ]]; then
    log "ERROR: expected binaries at $A_SRC/rust-mule and $B_SRC/rust-mule"
    echo "failed" >"$RUNNER_STATE_FILE"
    exit 1
  fi

  deadline="$(( $(ts_epoch) + duration_secs ))"
  log "soak-start duration_secs=$duration_secs deadline_epoch=$deadline"

  if ! start_nodes; then
    log "ERROR: failed to start nodes"
    echo "failed" >"$RUNNER_STATE_FILE"
    cleanup_runner
    exit 1
  fi
  if ! wait_ready; then
    log "ERROR: nodes failed readiness checks"
    echo "failed" >"$RUNNER_STATE_FILE"
    cleanup_runner
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
    run_round "$round"
    sleep "$WAIT_BETWEEN"
  done

  log "soak-finished rounds=$round"
  echo "completed" >"$RUNNER_STATE_FILE"
  cleanup_runner
}

start_background() {
  local duration_secs="${1:-1800}"
  ensure_dirs

  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      echo "runner already active pid=$pid"
      return 0
    fi
    rm -f "$RUNNER_PID_FILE"
  fi

  nohup bash -lc "cd '$ROOT' && '$0' run '$duration_secs'" >"$RUNNER_STDOUT_FILE" 2>&1 &
  echo $! >"$RUNNER_PID_FILE"
  log "runner started pid=$(cat "$RUNNER_PID_FILE") duration_secs=$duration_secs"
}

status_runner() {
  ensure_dirs
  if [[ -f "$RUNNER_PID_FILE" ]]; then
    local pid
    pid="$(cat "$RUNNER_PID_FILE")"
    if [[ -n "$pid" ]] && is_pid_alive "$pid"; then
      echo "status=running pid=$pid"
    else
      echo "status=stale_pid pid=${pid:-unknown}"
    fi
  else
    echo "status=not_running"
  fi

  if [[ -f "$RUNNER_STATE_FILE" ]]; then
    echo "runner_state=$(cat "$RUNNER_STATE_FILE")"
  fi
  if [[ -f "$LOG_DIR/a.pid" ]]; then
    local a_pid
    a_pid="$(cat "$LOG_DIR/a.pid")"
    if is_pid_alive "$a_pid"; then
      echo "node_a_pid=$a_pid alive=1"
    else
      echo "node_a_pid=$a_pid alive=0"
    fi
  fi
  if [[ -f "$LOG_DIR/b.pid" ]]; then
    local b_pid
    b_pid="$(cat "$LOG_DIR/b.pid")"
    if is_pid_alive "$b_pid"; then
      echo "node_b_pid=$b_pid alive=1"
    else
      echo "node_b_pid=$b_pid alive=0"
    fi
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

  stop_nodes
  stop_run_root_nodes
  stop_port_listeners
  echo "stopped" >"$RUNNER_STATE_FILE"
  log "runner stop requested"
}

collect_bundle() {
  ensure_dirs
  local bundle
  bundle="/tmp/rust-mule-soak-bg-$(date +%Y%m%d_%H%M%S).tar.gz"
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
  echo "usage: $0 {start [duration_secs]|run [duration_secs]|status|stop|collect}"
  exit 2
  ;;
esac
