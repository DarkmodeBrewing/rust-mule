#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"
DURATION_SECS="${DURATION_SECS:-1800}"
INTERVAL_SECS="${INTERVAL_SECS:-5}"

OUT_DIR="${OUT_DIR:-/tmp/rust-mule-kad-phase0-gate-$(date +%Y%m%d_%H%M%S)}"
BEFORE_OUT="${BEFORE_OUT:-$OUT_DIR/before.tsv}"
AFTER_OUT="${AFTER_OUT:-$OUT_DIR/after.tsv}"
COMPARE_OUT="${COMPARE_OUT:-$OUT_DIR/compare.tsv}"
GATE_OUT="${GATE_OUT:-$OUT_DIR/gate.tsv}"

BEFORE_SETUP_CMD="${BEFORE_SETUP_CMD:-}"
AFTER_SETUP_CMD="${AFTER_SETUP_CMD:-}"
SETUP_SETTLE_SECS="${SETUP_SETTLE_SECS:-10}"
READY_MAX_WAIT_SECS="${READY_MAX_WAIT_SECS:-120}"
READY_POLL_SECS="${READY_POLL_SECS:-2}"
READY_STABLE_SUCCESSES="${READY_STABLE_SUCCESSES:-3}"

ENFORCE_THRESHOLDS="${ENFORCE_THRESHOLDS:-1}"
MIN_SENT_REQS_TOTAL_RATIO="${MIN_SENT_REQS_TOTAL_RATIO:-0.90}"
MIN_RECV_RESS_TOTAL_RATIO="${MIN_RECV_RESS_TOTAL_RATIO:-0.90}"
MIN_TRACKED_OUT_MATCHED_TOTAL_RATIO="${MIN_TRACKED_OUT_MATCHED_TOTAL_RATIO:-0.90}"
MAX_TIMEOUTS_TOTAL_RATIO="${MAX_TIMEOUTS_TOTAL_RATIO:-1.10}"
MAX_OUTBOUND_SHAPER_DELAYED_TOTAL_RATIO="${MAX_OUTBOUND_SHAPER_DELAYED_TOTAL_RATIO:-1.25}"

usage() {
  cat <<'USAGE'
Usage:
  BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token \
    BEFORE_SETUP_CMD='<cmd-for-before-binary>' \
    AFTER_SETUP_CMD='<cmd-for-after-binary>' \
    DURATION_SECS=1800 INTERVAL_SECS=5 \
    bash scripts/test/kad_phase0_gate.sh

Optional env:
  OUT_DIR=/tmp/rust-mule-kad-phase0-gate-<ts>
  ENFORCE_THRESHOLDS=1|0
  READY_MAX_WAIT_SECS=120
  SETUP_SETTLE_SECS=10

Threshold env:
  MIN_SENT_REQS_TOTAL_RATIO=0.90
  MIN_RECV_RESS_TOTAL_RATIO=0.90
  MIN_TRACKED_OUT_MATCHED_TOTAL_RATIO=0.90
  MAX_TIMEOUTS_TOTAL_RATIO=1.10
  MAX_OUTBOUND_SHAPER_DELAYED_TOTAL_RATIO=1.25
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
baseline_script="$script_dir/kad_phase0_baseline.sh"
compare_script="$script_dir/kad_phase0_compare.sh"

if [[ ! -x "$baseline_script" ]]; then
  echo "missing baseline script: $baseline_script" >&2
  exit 1
fi
if [[ ! -x "$compare_script" ]]; then
  echo "missing compare script: $compare_script" >&2
  exit 1
fi
if [[ ! -f "$TOKEN_FILE" ]]; then
  echo "missing token file: $TOKEN_FILE" >&2
  exit 1
fi

TOKEN="$(cat "$TOKEN_FILE")"
mkdir -p "$OUT_DIR"

log() {
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*"
}

wait_until_ready() {
  local start now elapsed
  local stable=0
  start="$(date +%s)"
  while true; do
    local health_code status_code
    health_code="$(curl -sS -o /dev/null -w '%{http_code}' "$BASE_URL/api/v1/health" 2>/dev/null || echo 000)"
    status_code="$(curl -sS -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" "$BASE_URL/api/v1/status" 2>/dev/null || echo 000)"
    if [[ "$health_code" == "200" && "$status_code" == "200" ]]; then
      stable=$((stable + 1))
      if (( stable >= READY_STABLE_SUCCESSES )); then
        log "ready base_url=$BASE_URL health=$health_code status=$status_code stable=$stable"
        return 0
      fi
    else
      stable=0
    fi
    now="$(date +%s)"
    elapsed="$((now - start))"
    if (( elapsed >= READY_MAX_WAIT_SECS )); then
      echo "ERROR: readiness timeout after ${READY_MAX_WAIT_SECS}s (health=$health_code status=$status_code)" >&2
      return 1
    fi
    sleep "$READY_POLL_SECS"
  done
}

run_setup() {
  local phase="$1"
  local cmd="$2"
  if [[ -z "${cmd// }" ]]; then
    return 0
  fi
  log "running ${phase}_setup_cmd"
  bash -lc "$cmd"
  if (( SETUP_SETTLE_SECS > 0 )); then
    sleep "$SETUP_SETTLE_SECS"
  fi
  wait_until_ready
}

run_capture() {
  local label="$1"
  local out_file="$2"
  wait_until_ready
  log "capture_start label=$label out_file=$out_file duration_secs=$DURATION_SECS interval_secs=$INTERVAL_SECS"
  BASE_URL="$BASE_URL" \
  TOKEN_FILE="$TOKEN_FILE" \
  DURATION_SECS="$DURATION_SECS" \
  INTERVAL_SECS="$INTERVAL_SECS" \
  OUT_FILE="$out_file" \
    "$baseline_script"
  log "capture_done label=$label out_file=$out_file"
}

metric_field() {
  local metric="$1"
  local field_index="$2"
  awk -F '\t' -v m="$metric" -v f="$field_index" 'NR > 1 && $1 == m { print $f; exit }' "$COMPARE_OUT"
}

safe_ratio() {
  local before="$1"
  local after="$2"
  awk -v b="$before" -v a="$after" 'BEGIN {
    if (b <= 0) { print "nan"; exit }
    printf "%.6f", a / b
  }'
}

metric_total_rate() {
  local in_file="$1"
  local metric="$2"
  awk -F '\t' -v metric="$metric" '
    NR == 1 {
      for (i = 1; i <= NF; i++) {
        h[$i] = i;
      }
      next;
    }
    {
      u = $(h["uptime_secs"]) + 0;
      v = $(h[metric]) + 0;
      if (!seen) {
        first_u = u;
        first_v = v;
        seen = 1;
      }
      last_u = u;
      last_v = v;
    }
    END {
      if (!seen || last_u <= first_u || last_v < first_v) {
        print "nan";
        exit;
      }
      printf "%.6f", (last_v - first_v) / (last_u - first_u);
    }
  ' "$in_file"
}

check_min_ratio() {
  local check_name="$1"
  local metric="$2"
  local threshold="$3"
  local mode="${4:-avg}"
  local before after ratio pass

  if [[ "$mode" == "rate" ]]; then
    before="$(metric_total_rate "$BEFORE_OUT" "$metric")"
    after="$(metric_total_rate "$AFTER_OUT" "$metric")"
  else
    before="$(metric_field "$metric" 2)"
    after="$(metric_field "$metric" 3)"
  fi
  before="${before:-0}"
  after="${after:-0}"
  ratio="$(safe_ratio "$before" "$after")"

  if [[ "$ratio" == "nan" ]]; then
    pass="SKIP"
  else
    if awk -v r="$ratio" -v t="$threshold" 'BEGIN { exit !(r >= t) }'; then
      pass="PASS"
    else
      pass="FAIL"
      FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
  fi

  printf "%s\t%s\t%.6f\t%.6f\t%s\t%s\n" "$check_name" "$metric" "$before" "$after" "$ratio" "$threshold" >>"$GATE_OUT"
  log "gate $check_name metric=$metric mode=$mode before=$before after=$after ratio=$ratio threshold>=$threshold result=$pass"
}

check_max_ratio() {
  local check_name="$1"
  local metric="$2"
  local threshold="$3"
  local mode="${4:-avg}"
  local before after ratio pass

  if [[ "$mode" == "rate" ]]; then
    before="$(metric_total_rate "$BEFORE_OUT" "$metric")"
    after="$(metric_total_rate "$AFTER_OUT" "$metric")"
  else
    before="$(metric_field "$metric" 2)"
    after="$(metric_field "$metric" 3)"
  fi
  before="${before:-0}"
  after="${after:-0}"
  ratio="$(safe_ratio "$before" "$after")"

  if [[ "$ratio" == "nan" ]]; then
    pass="SKIP"
  else
    if awk -v r="$ratio" -v t="$threshold" 'BEGIN { exit !(r <= t) }'; then
      pass="PASS"
    else
      pass="FAIL"
      FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
  fi

  printf "%s\t%s\t%.6f\t%.6f\t%s\t%s\n" "$check_name" "$metric" "$before" "$after" "$ratio" "$threshold" >>"$GATE_OUT"
  log "gate $check_name metric=$metric mode=$mode before=$before after=$after ratio=$ratio threshold<=$threshold result=$pass"
}

log "phase0_gate_start out_dir=$OUT_DIR base_url=$BASE_URL token_file=$TOKEN_FILE"

run_setup "before" "$BEFORE_SETUP_CMD"
run_capture "before" "$BEFORE_OUT"

run_setup "after" "$AFTER_SETUP_CMD"
run_capture "after" "$AFTER_OUT"

"$compare_script" --before "$BEFORE_OUT" --after "$AFTER_OUT" >"$COMPARE_OUT"
log "compare_done compare_out=$COMPARE_OUT"

FAIL_COUNT=0
printf "check\tmetric\tbefore_avg\tafter_avg\tratio\tthreshold\n" >"$GATE_OUT"

check_min_ratio "throughput_sent_total" "sent_reqs_total" "$MIN_SENT_REQS_TOTAL_RATIO" "rate"
check_min_ratio "throughput_recv_total" "recv_ress_total" "$MIN_RECV_RESS_TOTAL_RATIO" "rate"
check_min_ratio "match_total" "tracked_out_matched_total" "$MIN_TRACKED_OUT_MATCHED_TOTAL_RATIO" "rate"
check_max_ratio "timeouts_total" "timeouts_total" "$MAX_TIMEOUTS_TOTAL_RATIO" "rate"
check_max_ratio "shaper_delayed_total" "outbound_shaper_delayed_total" "$MAX_OUTBOUND_SHAPER_DELAYED_TOTAL_RATIO" "rate"

if (( FAIL_COUNT > 0 )); then
  log "gate_result=FAIL fail_count=$FAIL_COUNT gate_file=$GATE_OUT compare_file=$COMPARE_OUT"
  if [[ "$ENFORCE_THRESHOLDS" == "1" ]]; then
    exit 3
  fi
else
  log "gate_result=PASS fail_count=0 gate_file=$GATE_OUT compare_file=$COMPARE_OUT"
fi

log "phase0_gate_done before=$BEFORE_OUT after=$AFTER_OUT compare=$COMPARE_OUT gate=$GATE_OUT"
