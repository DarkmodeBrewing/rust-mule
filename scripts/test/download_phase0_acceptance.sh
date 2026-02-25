#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"

GATE_DURATION_SECS="${GATE_DURATION_SECS:-1800}"
GATE_INTERVAL_SECS="${GATE_INTERVAL_SECS:-5}"
GATE_READY_MAX_WAIT_SECS="${GATE_READY_MAX_WAIT_SECS:-300}"

RUN_RESUME_SOAK="${RUN_RESUME_SOAK:-0}"
RESUME_ACTIVE_TRANSFER_TIMEOUT_SECS="${RESUME_ACTIVE_TRANSFER_TIMEOUT_SECS:-1800}"
RESUME_COMPLETION_TIMEOUT_SECS="${RESUME_COMPLETION_TIMEOUT_SECS:-3600}"
DOWNLOAD_FIXTURES_FILE="${DOWNLOAD_FIXTURES_FILE:-}"
FIXTURES_ONLY="${FIXTURES_ONLY:-0}"

RUN_KAD_LONGRUN="${RUN_KAD_LONGRUN:-0}"
LONGRUN_DURATION_SECS="${LONGRUN_DURATION_SECS:-21600}"
LONGRUN_INTERVAL_SECS="${LONGRUN_INTERVAL_SECS:-5}"

OUT_DIR="${OUT_DIR:-/tmp/rust-mule-download-phase0-accept-$(date +%Y%m%d_%H%M%S)}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE_SCRIPT="$SCRIPT_DIR/kad_phase0_gate.sh"
LONGRUN_SCRIPT="$SCRIPT_DIR/kad_phase0_longrun.sh"
RESUME_SCRIPT="$SCRIPT_DIR/download_resume_soak.sh"

usage() {
  cat <<'USAGE'
Usage:
  BASE_URL=http://127.0.0.1:17835 TOKEN_FILE=data/api.token \
    bash scripts/test/download_phase0_acceptance.sh

Optional env:
  GATE_DURATION_SECS=1800
  GATE_INTERVAL_SECS=5
  GATE_READY_MAX_WAIT_SECS=300
  RUN_RESUME_SOAK=0|1
  RESUME_ACTIVE_TRANSFER_TIMEOUT_SECS=1800
  RESUME_COMPLETION_TIMEOUT_SECS=3600
  RUN_KAD_LONGRUN=0|1
  LONGRUN_DURATION_SECS=21600
  LONGRUN_INTERVAL_SECS=5
  OUT_DIR=/tmp/rust-mule-download-phase0-accept-<timestamp>
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

require_file() {
  local p="$1"
  if [[ ! -f "$p" ]]; then
    echo "ERROR: missing file: $p" >&2
    exit 1
  fi
}

require_exec() {
  local p="$1"
  if [[ ! -x "$p" ]]; then
    echo "ERROR: missing executable script: $p" >&2
    exit 1
  fi
}

require_tool() {
  local t="$1"
  if ! command -v "$t" >/dev/null 2>&1; then
    echo "ERROR: missing required tool: $t" >&2
    exit 1
  fi
}

log() {
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*"
}

auth_get_to_file() {
  local path="$1"
  local out_file="$2"
  local code
  code="$(curl -sS -o "$out_file" -w '%{http_code}' -H "Authorization: Bearer $TOKEN" "$BASE_URL$path" || echo 000)"
  printf '%s\n' "$code"
}

capture_snapshots() {
  local label="$1"
  local dir="$OUT_DIR/snapshots/$label"
  mkdir -p "$dir"

  local health_code status_code downloads_code
  health_code="$(curl -sS -o "$dir/health.json" -w '%{http_code}' "$BASE_URL/api/v1/health" || echo 000)"
  status_code="$(auth_get_to_file "/api/v1/status" "$dir/status.json")"
  downloads_code="$(auth_get_to_file "/api/v1/downloads" "$dir/downloads.json")"

  cat > "$dir/http_codes.tsv" <<TSV
endpoint\thttp_code
/api/v1/health\t$health_code
/api/v1/status\t$status_code
/api/v1/downloads\t$downloads_code
TSV

  log "snapshot label=$label health=$health_code status=$status_code downloads=$downloads_code"
}

require_tool curl
require_file "$TOKEN_FILE"
require_exec "$GATE_SCRIPT"
require_exec "$LONGRUN_SCRIPT"
require_exec "$RESUME_SCRIPT"

TOKEN="$(cat "$TOKEN_FILE")"
mkdir -p "$OUT_DIR"

if [[ "$FIXTURES_ONLY" == "1" && -z "$DOWNLOAD_FIXTURES_FILE" ]]; then
  echo "ERROR: FIXTURES_ONLY=1 requires DOWNLOAD_FIXTURES_FILE to be set" >&2
  exit 1
fi
if [[ "$FIXTURES_ONLY" == "1" && ! -f "$DOWNLOAD_FIXTURES_FILE" ]]; then
  echo "ERROR: fixture file does not exist: $DOWNLOAD_FIXTURES_FILE" >&2
  exit 1
fi

log "phase0-accept-start out_dir=$OUT_DIR base_url=$BASE_URL token_file=$TOKEN_FILE"
if [[ -n "$DOWNLOAD_FIXTURES_FILE" ]]; then
  log "fixtures file=$DOWNLOAD_FIXTURES_FILE fixtures_only=$FIXTURES_ONLY"
fi

capture_snapshots "pre"

log "run gate (before/after consistency check)"
GATE_RC=0
if BASE_URL="$BASE_URL" \
  TOKEN_FILE="$TOKEN_FILE" \
  DURATION_SECS="$GATE_DURATION_SECS" \
  INTERVAL_SECS="$GATE_INTERVAL_SECS" \
  READY_MAX_WAIT_SECS="$GATE_READY_MAX_WAIT_SECS" \
  OUT_DIR="$OUT_DIR/kad-gate" \
  bash "$GATE_SCRIPT"; then
  GATE_RC=0
else
  GATE_RC=$?
fi

RESUME_RC=0
if [[ "$RUN_RESUME_SOAK" == "1" ]]; then
  log "run resume soak"
  if BASE_URL="$BASE_URL" \
    TOKEN_FILE="$TOKEN_FILE" \
    ACTIVE_TRANSFER_TIMEOUT_SECS="$RESUME_ACTIVE_TRANSFER_TIMEOUT_SECS" \
    COMPLETION_TIMEOUT_SECS="$RESUME_COMPLETION_TIMEOUT_SECS" \
    DOWNLOAD_FIXTURES_FILE="$DOWNLOAD_FIXTURES_FILE" \
    FIXTURES_ONLY="$FIXTURES_ONLY" \
    RESUME_OUT_DIR="$OUT_DIR/resume-soak" \
    bash "$RESUME_SCRIPT"; then
    RESUME_RC=0
  else
    RESUME_RC=$?
  fi
else
  log "skip resume soak (RUN_RESUME_SOAK=$RUN_RESUME_SOAK)"
fi

LONGRUN_RC=0
if [[ "$RUN_KAD_LONGRUN" == "1" ]]; then
  log "run kad longrun"
  if BASE_URL="$BASE_URL" \
    TOKEN_FILE="$TOKEN_FILE" \
    DURATION_SECS="$LONGRUN_DURATION_SECS" \
    INTERVAL_SECS="$LONGRUN_INTERVAL_SECS" \
    OUT_FILE="$OUT_DIR/kad-longrun.tsv" \
    bash "$LONGRUN_SCRIPT"; then
    LONGRUN_RC=0
  else
    LONGRUN_RC=$?
  fi
else
  log "skip kad longrun (RUN_KAD_LONGRUN=$RUN_KAD_LONGRUN)"
fi

capture_snapshots "post"

OVERALL_RC=0
if (( GATE_RC != 0 || RESUME_RC != 0 || LONGRUN_RC != 0 )); then
  OVERALL_RC=1
fi

cat > "$OUT_DIR/summary.txt" <<TXT
phase0_acceptance_out_dir=$OUT_DIR
base_url=$BASE_URL
token_file=$TOKEN_FILE

gate_rc=$GATE_RC
resume_soak_rc=$RESUME_RC
kad_longrun_rc=$LONGRUN_RC
overall_rc=$OVERALL_RC

gate_dir=$OUT_DIR/kad-gate
resume_dir=$OUT_DIR/resume-soak
longrun_tsv=$OUT_DIR/kad-longrun.tsv
snapshots_dir=$OUT_DIR/snapshots
TXT

log "phase0-accept-done overall_rc=$OVERALL_RC summary=$OUT_DIR/summary.txt"
exit "$OVERALL_RC"
