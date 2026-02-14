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
READY_TIMEOUT_SECS="${READY_TIMEOUT_SECS:-1200}"

RUNNER_PID_FILE="$RUN_ROOT/runner.pid"
RUNNER_LOG_FILE="$LOG_DIR/runner.log"
RUNNER_STATE_FILE="$RUN_ROOT/runner.state"
STOP_FILE="$RUN_ROOT/stop.requested"

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
ts_epoch() { date +%s; }
rand_hex16() { hexdump -n 16 -e '16/1 "%02x"' /dev/urandom; }

ensure_dirs() {
  mkdir -p "$RUN_ROOT" "$LOG_DIR"
}

is_pid_alive() {
  local pid="$1"
  kill -0 "$pid" 2>/dev/null
}

configure_b_instance() {
  sed -i 's/session_name = "rust-mule-b"/session_name = "rust-mule-b-soak"/' "$B_DIR/config.toml" || true
  sed -i 's/forward_port = 40000/forward_port = 40001/' "$B_DIR/config.toml" || true
  sed -i 's/udp_port = 4665/udp_port = 4666/' "$B_DIR/config.toml" || true
  sed -i 's/port = 17835/port = 17836/' "$B_DIR/config.toml" || true
}

start_nodes() {
  rm -rf "$A_DIR" "$B_DIR"
  cp -a "$A_SRC" "$A_DIR"
  cp -a "$B_SRC" "$B_DIR"

  configure_b_instance

  rm -f "$A_DIR/data/rust-mule.lock" "$B_DIR/data/rust-mule.lock"
  mkdir -p "$A_DIR/data/logs" "$B_DIR/data/logs"

  (cd "$A_DIR" && nohup ./rust-mule >"$LOG_DIR/a.out" 2>&1 & echo $! >"$LOG_DIR/a.pid")
  (cd "$B_DIR" && nohup ./rust-mule >"$LOG_DIR/b.out" 2>&1 & echo $! >"$LOG_DIR/b.pid")

  echo "$(ts) started A pid=$(cat "$LOG_DIR/a.pid") B pid=$(cat "$LOG_DIR/b.pid")" | tee -a "$RUNNER_LOG_FILE"
}

stop_nodes() {
  [[ -f "$LOG_DIR/a.pid" ]] && kill "$(cat "$LOG_DIR/a.pid")" 2>/dev/null || true
  [[ -f "$LOG_DIR/b.pid" ]] && kill "$(cat "$LOG_DIR/b.pid")" 2>/dev/null || true
  echo "$(ts) node stop requested" | tee -a "$RUNNER_LOG_FILE"
}

wait_ready() {
  local start now elapsed ta tb a_code b_code
  start="$(ts_epoch)"
  while true; do
    now="$(ts_epoch)"
    elapsed="$(( now - start ))"
    if (( elapsed > READY_TIMEOUT_SECS )); then
      echo "$(ts) ERROR: readiness timeout after ${READY_TIMEOUT_SECS}s" | tee -a "$RUNNER_LOG_FILE"
      return 1
    fi

    ta="$(cat "$A_DIR/data/api.token" 2>/dev/null || true)"
    tb="$(cat "$B_DIR/data/api.token" 2>/dev/null || true)"
    a_code="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $ta" "$A_URL/api/v1/status" || true)"
    b_code="$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $tb" "$B_URL/api/v1/status" || true)"
    echo "$(ts) ready-check elapsed=${elapsed}s A=$a_code B=$b_code" | tee -a "$RUNNER_LOG_FILE"

    if [[ "$a_code" == "200" && "$b_code" == "200" ]]; then
      return 0
    fi
    sleep 5
  done
}

run_round() {
  local round="$1"
  local file_id size ta tb a_pub b_srch b_get a_st b_st

  ta="$(cat "$A_DIR/data/api.token")"
  tb="$(cat "$B_DIR/data/api.token")"
  file_id="$(rand_hex16)"
  size="$(( (RANDOM % 5000000) + 1024 ))"

  echo "$(ts) round=$round file=$file_id size=$size" | tee -a "$RUNNER_LOG_FILE"

  a_pub="$(curl -sS -H "Authorization: Bearer $ta" -H 'content-type: application/json' \
    -d "{\"file_id_hex\":\"$file_id\",\"file_size\":$size}" \
    "$A_URL/api/v1/kad/publish_source" || true)"

  sleep "$WAIT_PUBLISH"

  b_srch="$(curl -sS -H "Authorization: Bearer $tb" -H 'content-type: application/json' \
    -d "{\"file_id_hex\":\"$file_id\",\"file_size\":$size}" \
    "$B_URL/api/v1/kad/search_sources" || true)"

  sleep "$WAIT_SEARCH"

  b_get="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/kad/sources/$file_id" || true)"
  a_st="$(curl -sS -H "Authorization: Bearer $ta" "$A_URL/api/v1/status" || true)"
  b_st="$(curl -sS -H "Authorization: Bearer $tb" "$B_URL/api/v1/status" || true)"

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$(ts)" "$round" "$file_id" "$a_pub" "$b_srch" "$b_get" >> "$LOG_DIR/rounds.tsv"
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

  trap 'echo "$(ts) runner interrupted" | tee -a "$RUNNER_LOG_FILE"; echo "stopped" > "$RUNNER_STATE_FILE"; cleanup_runner; exit 0' INT TERM

  if [[ ! -x "$A_SRC/rust-mule" || ! -x "$B_SRC/rust-mule" ]]; then
    echo "$(ts) ERROR: expected binaries at $A_SRC/rust-mule and $B_SRC/rust-mule" | tee -a "$RUNNER_LOG_FILE"
    echo "failed" >"$RUNNER_STATE_FILE"
    exit 1
  fi

  deadline="$(( $(ts_epoch) + duration_secs ))"
  echo "$(ts) soak-start duration_secs=$duration_secs deadline_epoch=$deadline" | tee -a "$RUNNER_LOG_FILE"

  start_nodes
  wait_ready

  round=0
  while true; do
    if [[ -f "$STOP_FILE" ]]; then
      echo "$(ts) stop marker detected" | tee -a "$RUNNER_LOG_FILE"
      break
    fi

    now="$(ts_epoch)"
    if (( now >= deadline )); then
      echo "$(ts) deadline reached" | tee -a "$RUNNER_LOG_FILE"
      break
    fi

    remaining="$(( deadline - now ))"
    round="$(( round + 1 ))"
    echo "$(ts) timer remaining_secs=$remaining round=$round" | tee -a "$RUNNER_LOG_FILE"
    run_round "$round"
    sleep "$WAIT_BETWEEN"
  done

  echo "$(ts) soak-finished rounds=$round" | tee -a "$RUNNER_LOG_FILE"
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

  nohup bash -lc "cd '$ROOT' && '$0' run '$duration_secs'" >>"$RUNNER_LOG_FILE" 2>&1 &
  echo $! >"$RUNNER_PID_FILE"
  echo "$(ts) runner started pid=$(cat "$RUNNER_PID_FILE") duration_secs=$duration_secs" | tee -a "$RUNNER_LOG_FILE"
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
    echo "node_a_pid=$(cat "$LOG_DIR/a.pid")"
  fi
  if [[ -f "$LOG_DIR/b.pid" ]]; then
    echo "node_b_pid=$(cat "$LOG_DIR/b.pid")"
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
  echo "stopped" >"$RUNNER_STATE_FILE"
  echo "$(ts) runner stop requested" | tee -a "$RUNNER_LOG_FILE"
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
