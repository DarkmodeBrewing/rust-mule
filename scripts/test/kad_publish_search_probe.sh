#!/usr/bin/env bash
set -euo pipefail

# Publish a source on node A, then search/poll on node B until sources appear.
#
# Usage:
#   scripts/test/kad_publish_search_probe.sh \
#     --file-id-hex 52be4bd97ca58ba9507877d71858de96 \
#     --file-size 2097152 \
#     --a-base-url http://127.0.0.1:17866 \
#     --a-token-file ../../mule-a/data/api.token \
#     --b-base-url http://127.0.0.1:17835 \
#     --b-token-file data/api.token
#
# Exit codes:
#   0 = sources observed on B
#   1 = timed out waiting for sources
#   2 = usage / precondition error

usage() {
  cat <<'EOF'
usage: scripts/test/kad_publish_search_probe.sh --file-id-hex HEX --file-size N --a-base-url URL --b-base-url URL [auth options] [timing options]

required:
  --file-id-hex HEX        32-hex MD4 file id
  --file-size N            file size used for publish/search payload
  --a-base-url URL         source publisher node API base URL
  --b-base-url URL         search/poll node API base URL

auth options:
  --a-token TOKEN          node A bearer token (literal token)
  --a-token-file PATH      node A token file (default: data/api.token)
  --b-token TOKEN          node B bearer token (literal token)
  --b-token-file PATH      node B token file (default: data/api.token)

timing options:
  --timeout-secs N         overall timeout (default: 900)
  --poll-secs N            poll interval (default: 5)
  --search-every N         issue B search every N polls (default: 2)
EOF
}

FILE_ID_HEX=""
FILE_SIZE=""
A_BASE_URL=""
B_BASE_URL=""
A_TOKEN=""
B_TOKEN=""
A_TOKEN_FILE="data/api.token"
B_TOKEN_FILE="data/api.token"
TIMEOUT_SECS=900
POLL_SECS=5
SEARCH_EVERY=2

while [[ $# -gt 0 ]]; do
  case "$1" in
  --file-id-hex)
    FILE_ID_HEX="$2"
    shift 2
    ;;
  --file-size)
    FILE_SIZE="$2"
    shift 2
    ;;
  --a-base-url)
    A_BASE_URL="$2"
    shift 2
    ;;
  --b-base-url)
    B_BASE_URL="$2"
    shift 2
    ;;
  --a-token)
    A_TOKEN="$2"
    shift 2
    ;;
  --a-token-file)
    A_TOKEN_FILE="$2"
    shift 2
    ;;
  --b-token)
    B_TOKEN="$2"
    shift 2
    ;;
  --b-token-file)
    B_TOKEN_FILE="$2"
    shift 2
    ;;
  --timeout-secs)
    TIMEOUT_SECS="$2"
    shift 2
    ;;
  --poll-secs)
    POLL_SECS="$2"
    shift 2
    ;;
  --search-every)
    SEARCH_EVERY="$2"
    shift 2
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "Unknown arg: $1" >&2
    usage
    exit 2
    ;;
  esac
done

if [[ -z "$FILE_ID_HEX" || -z "$FILE_SIZE" || -z "$A_BASE_URL" || -z "$B_BASE_URL" ]]; then
  echo "ERROR: missing required args" >&2
  usage
  exit 2
fi

if ! [[ "$FILE_ID_HEX" =~ ^[0-9a-fA-F]{32}$ ]]; then
  echo "ERROR: --file-id-hex must be 32 hex chars" >&2
  exit 2
fi

if ! [[ "$FILE_SIZE" =~ ^[0-9]+$ ]]; then
  echo "ERROR: --file-size must be a non-negative integer" >&2
  exit 2
fi

command -v curl >/dev/null 2>&1 || {
  echo "ERROR: curl is required" >&2
  exit 2
}
command -v jq >/dev/null 2>&1 || {
  echo "ERROR: jq is required" >&2
  exit 2
}

ts() { date +"%Y-%m-%dT%H:%M:%S%z"; }
log() { echo "$(ts) $*"; }

if [[ -z "$A_TOKEN" ]]; then
  A_TOKEN="$(cat "$A_TOKEN_FILE" 2>/dev/null || true)"
fi
if [[ -z "$B_TOKEN" ]]; then
  B_TOKEN="$(cat "$B_TOKEN_FILE" 2>/dev/null || true)"
fi
if [[ -z "$A_TOKEN" || -z "$B_TOKEN" ]]; then
  echo "ERROR: failed to resolve A/B tokens" >&2
  exit 2
fi

a_post() {
  local path="$1"
  local payload="$2"
  curl -sS \
    -H "Authorization: Bearer $A_TOKEN" \
    -H "Content-Type: application/json" \
    -d "$payload" \
    "$A_BASE_URL$path"
}

b_post() {
  local path="$1"
  local payload="$2"
  curl -sS \
    -H "Authorization: Bearer $B_TOKEN" \
    -H "Content-Type: application/json" \
    -d "$payload" \
    "$B_BASE_URL$path"
}

a_get() {
  local path="$1"
  curl -sS -H "Authorization: Bearer $A_TOKEN" "$A_BASE_URL$path"
}

b_get() {
  local path="$1"
  curl -sS -H "Authorization: Bearer $B_TOKEN" "$B_BASE_URL$path"
}

publish_payload="{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":$FILE_SIZE}"
search_payload="$publish_payload"

log "publish-start file_id_hex=$FILE_ID_HEX file_size=$FILE_SIZE a_base_url=$A_BASE_URL b_base_url=$B_BASE_URL"
pub_res="$(a_post "/api/v1/kad/publish_source" "$publish_payload" || true)"
log "publish-response $(echo "$pub_res" | tr -d '\n')"

start_epoch="$(date +%s)"
poll_idx=0

while true; do
  now_epoch="$(date +%s)"
  elapsed="$((now_epoch - start_epoch))"
  if (( elapsed > TIMEOUT_SECS )); then
    log "timeout elapsed=${elapsed}s no sources observed on B"
    exit 1
  fi

  poll_idx="$((poll_idx + 1))"

  if (( poll_idx % SEARCH_EVERY == 1 )); then
    s_res="$(b_post "/api/v1/kad/search_sources" "$search_payload" || true)"
    log "search-queued elapsed=${elapsed}s response=$(echo "$s_res" | tr -d '\n')"
  fi

  b_sources_json="$(b_get "/api/v1/kad/sources/$FILE_ID_HEX" || echo '{}')"
  b_sources_count="$(echo "$b_sources_json" | jq -r '(.sources // []) | length' 2>/dev/null || echo 0)"

  a_status="$(a_get "/api/v1/status" || echo '{}')"
  b_status="$(b_get "/api/v1/status" || echo '{}')"

  a_recv_pub="$(echo "$a_status" | jq -r '.recv_publish_source_reqs // 0' 2>/dev/null || echo 0)"
  a_sent_pub_res="$(echo "$a_status" | jq -r '.sent_publish_source_ress // 0' 2>/dev/null || echo 0)"
  a_recv_search_req="$(echo "$a_status" | jq -r '.recv_search_source_reqs // 0' 2>/dev/null || echo 0)"
  a_store_total="$(echo "$a_status" | jq -r '.source_store_entries_total // 0' 2>/dev/null || echo 0)"

  b_sent_search_req="$(echo "$b_status" | jq -r '.sent_search_source_reqs // 0' 2>/dev/null || echo 0)"
  b_recv_search_res="$(echo "$b_status" | jq -r '.recv_search_ress // 0' 2>/dev/null || echo 0)"
  b_store_total="$(echo "$b_status" | jq -r '.source_store_entries_total // 0' 2>/dev/null || echo 0)"

  log "poll elapsed=${elapsed}s b_sources=${b_sources_count} a{recv_pub=${a_recv_pub},sent_pub_res=${a_sent_pub_res},recv_search_req=${a_recv_search_req},store=${a_store_total}} b{sent_search_req=${b_sent_search_req},recv_search_res=${b_recv_search_res},store=${b_store_total}}"

  if [[ "$b_sources_count" =~ ^[0-9]+$ ]] && (( b_sources_count > 0 )); then
    log "success sources discovered on B count=$b_sources_count"
    echo "$b_sources_json" | jq .
    exit 0
  fi

  sleep "$POLL_SECS"
done
