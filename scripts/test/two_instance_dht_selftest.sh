#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/test/two_instance_dht_selftest.sh [options]

Runs a fair-ish two-instance test against mule-a and mule-b:

1) Publish keyword on A
2) Wait
3) Search keyword on A, then read results on A (tests "did any peer store it?")
4) Search keyword on B, then read results on B (tests "can B discover it via the network?")
5) Repeat with B->A

Notes:
- Keyword hashing is iMule-style: we hash the first extracted word.
- Endpoints are async (enqueue). This script adds delays to allow replies to arrive.

Options:
  --a-base-url URL         Default: http://127.0.0.1:17835
  --b-base-url URL         Default: http://127.0.0.1:17836
  --a-token-file PATH      Default: data/api.token
  --b-token-file PATH      Default: data/api.token
  --out-file PATH          Default: tmp/two_instance_dht_selftest_YYYYmmdd_HHMMSS.log
  --warmup-live N          Wait for each instance to have >=N live_10m peers before testing. Default: 3
  --warmup-timeout-secs N  Default: 900
  --warmup-check-secs N    Default: 30
  --warmup-stable-samples N  Require N consecutive successful samples. Default: 3

  --query TEXT             If set, used for BOTH A and B publish/search flows.
  --query-a TEXT           Default: mulea_test
  --query-b TEXT           Default: muleb_test

  --a-file-id-hex HEX      Default: 00112233445566778899aabbccddeeff
  --b-file-id-hex HEX      Default: 112233445566778899aabbccddeeff00
  --a-filename NAME        Default: mule-a-test.bin
  --b-filename NAME        Default: mule-b-test.bin
  --file-size N            Default: 123
  --file-type TYPE         Optional (e.g. Pro/Audio/Video/Image/Doc)

  --rounds N               Default: 2
  --wait-publish-secs N    Default: 60
  --wait-search-secs N     Default: 45
  --wait-sources-secs N    Default: 15
  --poll-interval-secs N   Default: 10 (used for keyword result polling)
  --pause-secs N           Default: 20
  --debug-lookup           Trigger one /api/v1/debug/lookup_once per instance (default: off)
  --peers-snapshot MODE    one of: each|first|none. Default: each
  --routing-snapshot MODE  one of: each|first|end|none. Default: each
EOF
}

A_BASE_URL="http://127.0.0.1:17835"
B_BASE_URL="http://127.0.0.1:17836"
A_TOKEN_FILE="data/api.token"
B_TOKEN_FILE="data/api.token"

QUERY=""
QUERY_A="mulea_test"
QUERY_B="muleb_test"

A_FILE_ID_HEX="00112233445566778899aabbccddeeff"
B_FILE_ID_HEX="112233445566778899aabbccddeeff00"
A_FILENAME="mule-a-test.bin"
B_FILENAME="mule-b-test.bin"
FILE_SIZE="123"
FILE_TYPE=""

ROUNDS="2"
WAIT_PUBLISH_SECS="60"
WAIT_SEARCH_SECS="45"
WAIT_SOURCES_SECS="15"
POLL_INTERVAL_SECS="10"
PAUSE_SECS="20"
DEBUG_LOOKUP="0"
PEERS_SNAPSHOT="each"
ROUTING_SNAPSHOT="each"

ts() { date +"%Y-%m-%d %H:%M:%S"; }
OUT_FILE="tmp/two_instance_dht_selftest_$(date +%Y%m%d_%H%M%S).log"

WARMUP_LIVE="3"
WARMUP_TIMEOUT_SECS="900"
WARMUP_CHECK_SECS="30"
WARMUP_STABLE_SAMPLES="3"

log() {
  # Send progress logs to stderr so callers can safely capture stdout (JSON) without losing logs.
  # Also tee progress logs into OUT_FILE for later analysis.
  echo "[$(ts)] $*" | tee -a "$OUT_FILE" >&2
}

append_blank_line() {
  # Ensure OUT_FILE stays readable even if some curl helpers don't end with a newline.
  echo | tee -a "$OUT_FILE" >/dev/null
}

extract_keyword_hex() {
  # Extract `"keyword_id_hex":"<32hex>"` from a JSON response.
  # Best-effort; returns empty string on failure.
  sed -n 's/.*"keyword_id_hex":"\([0-9a-fA-F]\{32\}\)".*/\1/p' | head -n 1 || true
}

extract_json_int_field() {
  # Extract an integer field from a flat JSON object (best-effort).
  # Usage: extract_json_int_field "$json" live_10m
  local json="$1"
  local field="$2"
  local m=""
  m="$(printf "%s" "$json" | grep -o "\"$field\":[0-9]*" | head -n 1 || true)"
  if [[ -z "$m" ]]; then
    echo ""
    return 0
  fi
  echo "${m#*:}"
}

extract_has_network_origin() {
  # Return "1" if any hit has origin=network in the JSON payload, else "0".
  local json="$1"
  if printf "%s" "$json" | grep -q "\"origin\":\"network\""; then
    echo "1"
  else
    echo "0"
  fi
}

poll_keyword_results() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local keyword_id_hex="$4"
  local timeout_secs="$5"
  local interval_secs="$6"

  log "Polling keyword results on $name keyword_id_hex=$keyword_id_hex (timeout=${timeout_secs}s, interval=${interval_secs}s)"
  local start now
  start="$(date +%s)"
  while true; do
    now="$(date +%s)"
    if (( now - start > timeout_secs )); then
      log "WARN: polling keyword results timed out on $name"
      get_keyword_results "$name" "$base_url" "$token_file" "$keyword_id_hex" >/dev/null
      return 0
    fi

    local json has_net
    json="$(scripts/docs/kad_keyword_results_get.sh \
      --base-url "$base_url" \
      --token-file "$token_file" \
      --keyword-id-hex "$keyword_id_hex" 2>/dev/null || true)"
    has_net="$(extract_has_network_origin "$json")"

    printf "%s\n" "$json" | tee -a "$OUT_FILE" >/dev/null
    append_blank_line

    if [[ "$has_net" == "1" ]]; then
      log "Detected network-origin hit on $name"
      return 0
    fi

    sleep "$interval_secs"
  done
}

wait_for_warmup() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local need_live="$4"
  local timeout_secs="$5"
  local check_secs="$6"
  local stable_samples="$7"

  if [[ "$need_live" -le 0 ]]; then
    return 0
  fi

  log "Warmup $name: waiting for live_10m >= $need_live (stable $stable_samples samples, timeout ${timeout_secs}s, check ${check_secs}s)"
  local start now consec=0
  start="$(date +%s)"
  while true; do
    now="$(date +%s)"
    if (( now - start > timeout_secs )); then
      log "WARN: Warmup $name timed out; continuing anyway"
      return 0
    fi

    local s live live10 routing
    s="$(scripts/docs/status.sh --base-url "$base_url" --token-file "$token_file" 2>/dev/null || true)"
    live10="$(extract_json_int_field "$s" live_10m)"
    live="$(extract_json_int_field "$s" live)"
    routing="$(extract_json_int_field "$s" routing)"
    live10="${live10:-0}"
    live="${live:-0}"
    routing="${routing:-0}"

    if [[ "$live10" -ge "$need_live" ]]; then
      consec=$((consec + 1))
    else
      consec=0
    fi

    log "Warmup $name: routing=$routing live=$live live_10m=$live10 consec=$consec/$stable_samples"
    if (( consec >= stable_samples )); then
      log "Warmup $name complete"
      return 0
    fi

    sleep "$check_secs"
  done
}

healthcheck() {
  local name="$1"
  local base_url="$2"
  log "Healthcheck $name ($base_url)"
  scripts/docs/health.sh --base-url "$base_url" >/dev/null
}

status_snapshot() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  log "Status $name ($base_url)"
  scripts/docs/status.sh --base-url "$base_url" --token-file "$token_file" | tee -a "$OUT_FILE" || true
  append_blank_line
}

publish_keyword() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local query="$4"
  local file_id_hex="$5"
  local filename="$6"
  local file_size="$7"
  local file_type="$8"

  log "Publish keyword on $name query='$query' file_id_hex=$file_id_hex filename='$filename' size=$file_size"
  if [[ -n "$file_type" ]]; then
    scripts/docs/kad_publish_keyword.sh \
      --base-url "$base_url" \
      --token-file "$token_file" \
      --query "$query" \
      --file-id-hex "$file_id_hex" \
      --filename "$filename" \
      --file-size "$file_size" \
      --file-type "$file_type" \
      | tee -a "$OUT_FILE"
  else
    scripts/docs/kad_publish_keyword.sh \
      --base-url "$base_url" \
      --token-file "$token_file" \
      --query "$query" \
      --file-id-hex "$file_id_hex" \
      --filename "$filename" \
      --file-size "$file_size" \
      | tee -a "$OUT_FILE"
  fi
  append_blank_line
}

search_keyword() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local keyword_id_hex="$4"

  log "Search keyword on $name keyword_id_hex=$keyword_id_hex"
  scripts/docs/kad_search_keyword.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --keyword-id-hex "$keyword_id_hex" \
    | tee -a "$OUT_FILE"
  append_blank_line
}

get_keyword_results() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local keyword_id_hex="$4"

  log "Get keyword results on $name keyword_id_hex=$keyword_id_hex"
  scripts/docs/kad_keyword_results_get.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --keyword-id-hex "$keyword_id_hex" \
    | tee -a "$OUT_FILE"
  append_blank_line
}

publish_source() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local file_id_hex="$4"
  local file_size="$5"

  log "Publish source on $name file_id_hex=$file_id_hex size=$file_size"
  scripts/docs/kad_publish_source.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --file-id-hex "$file_id_hex" \
    --file-size "$file_size" \
    | tee -a "$OUT_FILE"
  append_blank_line
}

search_sources() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local file_id_hex="$4"
  local file_size="$5"

  log "Search sources on $name file_id_hex=$file_id_hex size=$file_size"
  scripts/docs/kad_search_sources.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --file-id-hex "$file_id_hex" \
    --file-size "$file_size" \
    | tee -a "$OUT_FILE"
  append_blank_line
}

get_sources() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local file_id_hex="$4"

  log "Get sources on $name file_id_hex=$file_id_hex"
  scripts/docs/kad_sources_get.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --file-id-hex "$file_id_hex" \
    | tee -a "$OUT_FILE"
  append_blank_line
}

peers_snapshot() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"

  case "$PEERS_SNAPSHOT" in
    none) return 0 ;;
    first)
      if [[ -n "${PEERS_SNAPSHOT_DONE:-}" ]]; then
        return 0
      fi
      ;;
  esac

  log "Peers $name ($base_url)"
  bash scripts/docs/kad_peers_get.sh --base-url "$base_url" --token-file "$token_file" | tee -a "$OUT_FILE" || true
  append_blank_line
  PEERS_SNAPSHOT_DONE="1"
}

routing_should_snapshot() {
  local mode="$1"
  case "$mode" in
    none) return 1 ;;
    first)
      if [[ -n "${ROUTING_SNAPSHOT_DONE:-}" ]]; then
        return 1
      fi
      ;;
    end) return 1 ;;
  esac
  return 0
}

routing_summary_snapshot() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"

  if ! routing_should_snapshot "$ROUTING_SNAPSHOT"; then
    return 0
  fi

  log "Routing summary $name ($base_url)"
  bash scripts/docs/debug_routing_summary.sh --base-url "$base_url" --token-file "$token_file" | tee -a "$OUT_FILE" || true
  append_blank_line
  ROUTING_SNAPSHOT_DONE="1"
}

routing_buckets_snapshot() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"

  if ! routing_should_snapshot "$ROUTING_SNAPSHOT"; then
    return 0
  fi

  log "Routing buckets $name ($base_url)"
  bash scripts/docs/debug_routing_buckets.sh --base-url "$base_url" --token-file "$token_file" | tee -a "$OUT_FILE" || true
  append_blank_line
  ROUTING_SNAPSHOT_DONE="1"
}

debug_lookup_once() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"

  log "Debug lookup $name ($base_url)"
  bash scripts/docs/debug_lookup_once.sh --base-url "$base_url" --token-file "$token_file" | tee -a "$OUT_FILE" || true
  append_blank_line
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --a-base-url) A_BASE_URL="$2"; shift 2 ;;
    --b-base-url) B_BASE_URL="$2"; shift 2 ;;
    --a-token-file) A_TOKEN_FILE="$2"; shift 2 ;;
    --b-token-file) B_TOKEN_FILE="$2"; shift 2 ;;
    --out-file) OUT_FILE="$2"; shift 2 ;;

    --warmup-live) WARMUP_LIVE="$2"; shift 2 ;;
    --warmup-timeout-secs) WARMUP_TIMEOUT_SECS="$2"; shift 2 ;;
    --warmup-check-secs) WARMUP_CHECK_SECS="$2"; shift 2 ;;
    --warmup-stable-samples) WARMUP_STABLE_SAMPLES="$2"; shift 2 ;;

    --query) QUERY="$2"; shift 2 ;;
    --query-a) QUERY_A="$2"; shift 2 ;;
    --query-b) QUERY_B="$2"; shift 2 ;;

    --a-file-id-hex) A_FILE_ID_HEX="$2"; shift 2 ;;
    --b-file-id-hex) B_FILE_ID_HEX="$2"; shift 2 ;;
    --a-filename) A_FILENAME="$2"; shift 2 ;;
    --b-filename) B_FILENAME="$2"; shift 2 ;;
    --file-size) FILE_SIZE="$2"; shift 2 ;;
    --file-type) FILE_TYPE="$2"; shift 2 ;;

    --rounds) ROUNDS="$2"; shift 2 ;;
    --wait-publish-secs) WAIT_PUBLISH_SECS="$2"; shift 2 ;;
    --wait-search-secs) WAIT_SEARCH_SECS="$2"; shift 2 ;;
    --wait-sources-secs) WAIT_SOURCES_SECS="$2"; shift 2 ;;
    --poll-interval-secs) POLL_INTERVAL_SECS="$2"; shift 2 ;;
    --pause-secs) PAUSE_SECS="$2"; shift 2 ;;
    --debug-lookup) DEBUG_LOOKUP="1"; shift 1 ;;
    --peers-snapshot) PEERS_SNAPSHOT="$2"; shift 2 ;;
    --routing-snapshot) ROUTING_SNAPSHOT="$2"; shift 2 ;;

    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

mkdir -p "$(dirname "$OUT_FILE")"

log "Script: $0 $*"
log "OUT_FILE=$OUT_FILE"
log "A_BASE_URL=$A_BASE_URL A_TOKEN_FILE=$A_TOKEN_FILE"
log "B_BASE_URL=$B_BASE_URL B_TOKEN_FILE=$B_TOKEN_FILE"
log "ROUNDS=$ROUNDS WAIT_PUBLISH_SECS=$WAIT_PUBLISH_SECS WAIT_SEARCH_SECS=$WAIT_SEARCH_SECS PAUSE_SECS=$PAUSE_SECS"
log "WAIT_SOURCES_SECS=$WAIT_SOURCES_SECS"
log "POLL_INTERVAL_SECS=$POLL_INTERVAL_SECS PEERS_SNAPSHOT=$PEERS_SNAPSHOT"
log "ROUTING_SNAPSHOT=$ROUTING_SNAPSHOT"
log "WARMUP_LIVE=$WARMUP_LIVE WARMUP_TIMEOUT_SECS=$WARMUP_TIMEOUT_SECS WARMUP_CHECK_SECS=$WARMUP_CHECK_SECS WARMUP_STABLE_SAMPLES=$WARMUP_STABLE_SAMPLES"
log "DEBUG_LOOKUP=$DEBUG_LOOKUP"

if [[ -n "$QUERY" ]]; then
  QUERY_A="$QUERY"
  QUERY_B="$QUERY"
fi

healthcheck "A" "$A_BASE_URL"
healthcheck "B" "$B_BASE_URL"

status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
peers_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
peers_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
routing_summary_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
routing_summary_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
routing_buckets_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
routing_buckets_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

if [[ "$DEBUG_LOOKUP" -eq 1 ]]; then
  debug_lookup_once "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  debug_lookup_once "B" "$B_BASE_URL" "$B_TOKEN_FILE"
fi

wait_for_warmup "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$WARMUP_LIVE" "$WARMUP_TIMEOUT_SECS" "$WARMUP_CHECK_SECS" "$WARMUP_STABLE_SAMPLES"
wait_for_warmup "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$WARMUP_LIVE" "$WARMUP_TIMEOUT_SECS" "$WARMUP_CHECK_SECS" "$WARMUP_STABLE_SAMPLES"

routing_summary_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
routing_summary_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
routing_buckets_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
routing_buckets_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

for ((i=1; i<=ROUNDS; i++)); do
  log "=== Round $i/$ROUNDS: A -> (A,B) ==="

  RESP_A="$(publish_keyword "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$QUERY_A" "$A_FILE_ID_HEX" "$A_FILENAME" "$FILE_SIZE" "$FILE_TYPE")"
  KEY_A="$(printf "%s" "$RESP_A" | extract_keyword_hex)"
  if [[ -z "$KEY_A" ]]; then
    log "WARN: could not extract keyword_id_hex from A publish response; falling back to searching by query on both sides"
    # Using query here is weaker because both sides must tokenize/hash identically; still better than stopping.
    KEY_A=""
  else
    log "A publish returned keyword_id_hex=$KEY_A"
  fi

  log "Waiting ${WAIT_PUBLISH_SECS}s for publish to propagate..."
  sleep "$WAIT_PUBLISH_SECS"

  if [[ -n "$KEY_A" ]]; then
    search_keyword "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_A" >/dev/null
    poll_keyword_results "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_A" "$WAIT_SEARCH_SECS" "$POLL_INTERVAL_SECS"

    search_keyword "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_A" >/dev/null
    poll_keyword_results "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_A" "$WAIT_SEARCH_SECS" "$POLL_INTERVAL_SECS"
  else
    # Query-based fallback.
    log "Search keyword on A by query='$QUERY_A' (fallback)"
    scripts/docs/kad_search_keyword.sh --base-url "$A_BASE_URL" --token-file "$A_TOKEN_FILE" --query "$QUERY_A" >/dev/null
    sleep "$WAIT_SEARCH_SECS"
    log "NOTE: keyword_id_hex unknown; use /api/v1/kad/search_keyword response to fetch results"
  fi

  status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
  peers_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  peers_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
  routing_summary_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  routing_summary_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

  log "Pausing ${PAUSE_SECS}s..."
  sleep "$PAUSE_SECS"

  log "=== Round $i/$ROUNDS: A source -> B ==="

  publish_source "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$A_FILE_ID_HEX" "$FILE_SIZE"
  log "Waiting ${WAIT_PUBLISH_SECS}s for publish source to propagate..."
  sleep "$WAIT_PUBLISH_SECS"

  search_sources "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$A_FILE_ID_HEX" "$FILE_SIZE" >/dev/null
  log "Waiting ${WAIT_SOURCES_SECS}s for source replies (B)..."
  sleep "$WAIT_SOURCES_SECS"
  get_sources "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$A_FILE_ID_HEX"

  status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
  peers_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  peers_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

  log "Pausing ${PAUSE_SECS}s..."
  sleep "$PAUSE_SECS"

  log "=== Round $i/$ROUNDS: B -> (B,A) ==="

  RESP_B="$(publish_keyword "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$QUERY_B" "$B_FILE_ID_HEX" "$B_FILENAME" "$FILE_SIZE" "$FILE_TYPE")"
  KEY_B="$(printf "%s" "$RESP_B" | extract_keyword_hex)"
  if [[ -z "$KEY_B" ]]; then
    log "WARN: could not extract keyword_id_hex from B publish response; falling back to searching by query"
    KEY_B=""
  else
    log "B publish returned keyword_id_hex=$KEY_B"
  fi

  log "Waiting ${WAIT_PUBLISH_SECS}s for publish to propagate..."
  sleep "$WAIT_PUBLISH_SECS"

  if [[ -n "$KEY_B" ]]; then
    search_keyword "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_B" >/dev/null
    poll_keyword_results "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_B" "$WAIT_SEARCH_SECS" "$POLL_INTERVAL_SECS"

    search_keyword "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_B" >/dev/null
    poll_keyword_results "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_B" "$WAIT_SEARCH_SECS" "$POLL_INTERVAL_SECS"
  else
    log "Search keyword on B by query='$QUERY_B' (fallback)"
    scripts/docs/kad_search_keyword.sh --base-url "$B_BASE_URL" --token-file "$B_TOKEN_FILE" --query "$QUERY_B" >/dev/null
    sleep "$WAIT_SEARCH_SECS"
    log "NOTE: keyword_id_hex unknown; use /api/v1/kad/search_keyword response to fetch results"
  fi

  status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
  peers_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  peers_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

  log "Pausing ${PAUSE_SECS}s..."
  sleep "$PAUSE_SECS"

  log "=== Round $i/$ROUNDS: B source -> A ==="

  publish_source "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$B_FILE_ID_HEX" "$FILE_SIZE"
  log "Waiting ${WAIT_PUBLISH_SECS}s for publish source to propagate..."
  sleep "$WAIT_PUBLISH_SECS"

  search_sources "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$B_FILE_ID_HEX" "$FILE_SIZE" >/dev/null
  log "Waiting ${WAIT_SOURCES_SECS}s for source replies (A)..."
  sleep "$WAIT_SOURCES_SECS"
  get_sources "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$B_FILE_ID_HEX"

  status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"
  peers_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  peers_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

  log "Pausing ${PAUSE_SECS}s..."
  sleep "$PAUSE_SECS"
done

if [[ "$ROUTING_SNAPSHOT" == "end" ]]; then
  log "Final routing summary A ($A_BASE_URL)"
  bash scripts/docs/debug_routing_summary.sh --base-url "$A_BASE_URL" --token-file "$A_TOKEN_FILE" | tee -a "$OUT_FILE" || true
  append_blank_line
  log "Final routing summary B ($B_BASE_URL)"
  bash scripts/docs/debug_routing_summary.sh --base-url "$B_BASE_URL" --token-file "$B_TOKEN_FILE" | tee -a "$OUT_FILE" || true
  append_blank_line
  log "Final routing buckets A ($A_BASE_URL)"
  bash scripts/docs/debug_routing_buckets.sh --base-url "$A_BASE_URL" --token-file "$A_TOKEN_FILE" | tee -a "$OUT_FILE" || true
  append_blank_line
  log "Final routing buckets B ($B_BASE_URL)"
  bash scripts/docs/debug_routing_buckets.sh --base-url "$B_BASE_URL" --token-file "$B_TOKEN_FILE" | tee -a "$OUT_FILE" || true
  append_blank_line
fi

log "Done."
