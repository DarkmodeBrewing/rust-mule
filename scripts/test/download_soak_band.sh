#!/usr/bin/env bash
set -euo pipefail

# Runs download soak scenarios sequentially ("in band"), with auto stop/collect.
#
# Default order and durations:
#   1) integrity    (3600s)
#   2) single_e2e   (3600s)
#   3) concurrency  (7200s)
#   4) long_churn   (7200s)
#
# Usage:
#   scripts/test/download_soak_band.sh
#   BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token scripts/test/download_soak_band.sh

BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"
POLL_SECS="${POLL_SECS:-30}"
STOP_GRACE_SECS="${STOP_GRACE_SECS:-60}"
OUT_DIR="${OUT_DIR:-/tmp/rust-mule-download-soak-band-$(date +%Y%m%d_%H%M%S)}"

INTEGRITY_SECS="${INTEGRITY_SECS:-3600}"
SINGLE_E2E_SECS="${SINGLE_E2E_SECS:-3600}"
CONCURRENCY_SECS="${CONCURRENCY_SECS:-7200}"
LONG_CHURN_SECS="${LONG_CHURN_SECS:-7200}"

CONCURRENCY_TARGET="${CONCURRENCY_TARGET:-20}"
CHURN_MAX_QUEUE="${CHURN_MAX_QUEUE:-25}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
log() { echo "$(ts) $*"; }

require_file() {
  local f="$1"
  [[ -f "$f" ]] || {
    echo "ERROR: missing file: $f" >&2
    exit 1
  }
}

status_field() {
  local output="$1"
  local key="$2"
  echo "$output" | awk -F '=' -v k="$key" '$1==k {print $2}' | tail -n 1
}

wait_for_runner() {
  local wrapper="$1"
  local duration="$2"
  local scenario="$3"
  local deadline now status_out status_value state_value

  deadline="$(( $(date +%s) + duration + STOP_GRACE_SECS ))"
  while true; do
    now="$(date +%s)"
    status_out="$("$wrapper" status 2>&1 || true)"
    status_value="$(status_field "$status_out" "status")"
    state_value="$(status_field "$status_out" "runner_state")"

    log "scenario=$scenario poll status=${status_value:-unknown} state=${state_value:-unknown}"
    printf '%s\t%s\t%s\t%s\n' "$(ts)" "$scenario" "${status_value:-unknown}" "${state_value:-unknown}" >>"$OUT_DIR/status.tsv"

    if [[ "$status_value" != "running" ]]; then
      return 0
    fi

    if (( now >= deadline )); then
      log "scenario=$scenario timeout waiting for completion; forcing stop"
      "$wrapper" stop >/dev/null 2>&1 || true
      return 0
    fi
    sleep "$POLL_SECS"
  done
}

run_one() {
  local scenario="$1"
  local wrapper="$2"
  local duration="$3"
  shift 3
  local extra_env=("$@")

  local tarball status_out state_value result
  result="completed"

  log "scenario=$scenario start duration_secs=$duration"
  env BASE_URL="$BASE_URL" TOKEN_FILE="$TOKEN_FILE" "${extra_env[@]}" "$wrapper" start "$duration"

  wait_for_runner "$wrapper" "$duration" "$scenario"
  status_out="$(env BASE_URL="$BASE_URL" TOKEN_FILE="$TOKEN_FILE" "${extra_env[@]}" "$wrapper" status 2>&1 || true)"
  state_value="$(status_field "$status_out" "runner_state")"
  log "scenario=$scenario final_state=${state_value:-unknown}"

  # Idempotent cleanup: ensure runner is not left behind.
  env BASE_URL="$BASE_URL" TOKEN_FILE="$TOKEN_FILE" "${extra_env[@]}" "$wrapper" stop >/dev/null 2>&1 || true

  tarball="$(env BASE_URL="$BASE_URL" TOKEN_FILE="$TOKEN_FILE" "${extra_env[@]}" "$wrapper" collect)"
  if [[ -f "$tarball" ]]; then
    cp -f "$tarball" "$OUT_DIR/"
    log "scenario=$scenario collected=$(basename "$tarball")"
  else
    log "scenario=$scenario collect_failed tarball_path=$tarball"
    result="collect_failed"
  fi

  if [[ "$state_value" == "failed" ]]; then
    result="failed"
  fi

  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$(ts)" "$scenario" "$duration" "${state_value:-unknown}" "$result" >>"$OUT_DIR/results.tsv"
}

main() {
  mkdir -p "$OUT_DIR"
  : >"$OUT_DIR/results.tsv"
  : >"$OUT_DIR/status.tsv"

  require_file "$TOKEN_FILE"
  require_file "$SCRIPT_DIR/download_soak_integrity_bg.sh"
  require_file "$SCRIPT_DIR/download_soak_single_e2e_bg.sh"
  require_file "$SCRIPT_DIR/download_soak_concurrency_bg.sh"
  require_file "$SCRIPT_DIR/download_soak_long_churn_bg.sh"

  log "band-start out_dir=$OUT_DIR base_url=$BASE_URL token_file=$TOKEN_FILE"

  run_one "integrity" \
    "$SCRIPT_DIR/download_soak_integrity_bg.sh" \
    "$INTEGRITY_SECS"

  run_one "single_e2e" \
    "$SCRIPT_DIR/download_soak_single_e2e_bg.sh" \
    "$SINGLE_E2E_SECS"

  run_one "concurrency" \
    "$SCRIPT_DIR/download_soak_concurrency_bg.sh" \
    "$CONCURRENCY_SECS" \
    "CONCURRENCY_TARGET=$CONCURRENCY_TARGET"

  run_one "long_churn" \
    "$SCRIPT_DIR/download_soak_long_churn_bg.sh" \
    "$LONG_CHURN_SECS" \
    "CHURN_MAX_QUEUE=$CHURN_MAX_QUEUE"

  log "band-finished out_dir=$OUT_DIR"
  log "results_file=$OUT_DIR/results.tsv"
}

main "$@"
