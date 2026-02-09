#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/two_instance_dht_selftest.sh [options]

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
  --wait-search-secs N     Default: 15
  --pause-secs N           Default: 20
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
WAIT_SEARCH_SECS="15"
PAUSE_SECS="20"

ts() { date +"%Y-%m-%d %H:%M:%S"; }

log() {
  echo "[$(ts)] $*"
}

extract_keyword_hex() {
  # Extract `"keyword_id_hex":"<32hex>"` from a JSON response.
  # Best-effort; returns empty string on failure.
  sed -n 's/.*"keyword_id_hex":"\([0-9a-fA-F]\{32\}\)".*/\1/p' | head -n 1 || true
}

healthcheck() {
  local name="$1"
  local base_url="$2"
  log "Healthcheck $name ($base_url)"
  docs/scripts/health.sh --base-url "$base_url" >/dev/null
}

status_snapshot() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  log "Status $name ($base_url)"
  docs/scripts/status.sh --base-url "$base_url" --token-file "$token_file" || true
  echo
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
    docs/scripts/kad_publish_keyword.sh \
      --base-url "$base_url" \
      --token-file "$token_file" \
      --query "$query" \
      --file-id-hex "$file_id_hex" \
      --filename "$filename" \
      --file-size "$file_size" \
      --file-type "$file_type"
  else
    docs/scripts/kad_publish_keyword.sh \
      --base-url "$base_url" \
      --token-file "$token_file" \
      --query "$query" \
      --file-id-hex "$file_id_hex" \
      --filename "$filename" \
      --file-size "$file_size"
  fi
  echo
}

search_keyword() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local keyword_id_hex="$4"

  log "Search keyword on $name keyword_id_hex=$keyword_id_hex"
  docs/scripts/kad_search_keyword.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --keyword-id-hex "$keyword_id_hex"
  echo
}

get_keyword_results() {
  local name="$1"
  local base_url="$2"
  local token_file="$3"
  local keyword_id_hex="$4"

  log "Get keyword results on $name keyword_id_hex=$keyword_id_hex"
  docs/scripts/kad_keyword_results_get.sh \
    --base-url "$base_url" \
    --token-file "$token_file" \
    --keyword-id-hex "$keyword_id_hex"
  echo
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --a-base-url) A_BASE_URL="$2"; shift 2 ;;
    --b-base-url) B_BASE_URL="$2"; shift 2 ;;
    --a-token-file) A_TOKEN_FILE="$2"; shift 2 ;;
    --b-token-file) B_TOKEN_FILE="$2"; shift 2 ;;

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
    --pause-secs) PAUSE_SECS="$2"; shift 2 ;;

    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -n "$QUERY" ]]; then
  QUERY_A="$QUERY"
  QUERY_B="$QUERY"
fi

healthcheck "A" "$A_BASE_URL"
healthcheck "B" "$B_BASE_URL"

status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

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
    log "Waiting ${WAIT_SEARCH_SECS}s for search replies (A)..."
    sleep "$WAIT_SEARCH_SECS"
    get_keyword_results "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_A"

    search_keyword "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_A" >/dev/null
    log "Waiting ${WAIT_SEARCH_SECS}s for search replies (B)..."
    sleep "$WAIT_SEARCH_SECS"
    get_keyword_results "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_A"
  else
    # Query-based fallback.
    log "Search keyword on A by query='$QUERY_A' (fallback)"
    docs/scripts/kad_search_keyword.sh --base-url "$A_BASE_URL" --token-file "$A_TOKEN_FILE" --query "$QUERY_A" >/dev/null
    sleep "$WAIT_SEARCH_SECS"
    log "NOTE: keyword_id_hex unknown; use /kad/search_keyword response to fetch results"
  fi

  status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

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
    log "Waiting ${WAIT_SEARCH_SECS}s for search replies (B)..."
    sleep "$WAIT_SEARCH_SECS"
    get_keyword_results "B" "$B_BASE_URL" "$B_TOKEN_FILE" "$KEY_B"

    search_keyword "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_B" >/dev/null
    log "Waiting ${WAIT_SEARCH_SECS}s for search replies (A)..."
    sleep "$WAIT_SEARCH_SECS"
    get_keyword_results "A" "$A_BASE_URL" "$A_TOKEN_FILE" "$KEY_B"
  else
    log "Search keyword on B by query='$QUERY_B' (fallback)"
    docs/scripts/kad_search_keyword.sh --base-url "$B_BASE_URL" --token-file "$B_TOKEN_FILE" --query "$QUERY_B" >/dev/null
    sleep "$WAIT_SEARCH_SECS"
    log "NOTE: keyword_id_hex unknown; use /kad/search_keyword response to fetch results"
  fi

  status_snapshot "A" "$A_BASE_URL" "$A_TOKEN_FILE"
  status_snapshot "B" "$B_BASE_URL" "$B_TOKEN_FILE"

  log "Pausing ${PAUSE_SECS}s..."
  sleep "$PAUSE_SECS"
done

log "Done."

