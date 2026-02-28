#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/docs/download_create_from_hash.sh --file-id-hex HEX --file-size N --file-name NAME [--search-first 1|0] [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  Optional POST /api/v1/kad/search_sources
  POST /api/v1/downloads

Options:
  --file-id-hex HEX    32 hex chars (MD4 file id)
  --file-size N        File size in bytes
  --file-name NAME     Local/incoming file name
  --search-first 1|0   Queue KAD source search before download create (default: 1)
  --base-url URL       Default: http://127.0.0.1:17835
  --token TOKEN        Bearer token (overrides --token-file)
  --token-file PATH    Default: data/api.token
USAGE
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
FILE_ID_HEX=""
FILE_SIZE=""
FILE_NAME=""
SEARCH_FIRST="1"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file-id-hex) FILE_ID_HEX="$2"; shift 2 ;;
    --file-size) FILE_SIZE="$2"; shift 2 ;;
    --file-name) FILE_NAME="$2"; shift 2 ;;
    --search-first) SEARCH_FIRST="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$FILE_ID_HEX" || -z "$FILE_SIZE" || -z "$FILE_NAME" ]]; then
  echo "Missing required args: --file-id-hex --file-size --file-name" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

if [[ "$SEARCH_FIRST" == "1" ]]; then
  curl -sS \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":$FILE_SIZE}" \
    "$BASE_URL/api/v1/kad/search_sources"
  echo
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"file_name\":\"$FILE_NAME\",\"file_size\":$FILE_SIZE,\"file_hash_md4_hex\":\"$FILE_ID_HEX\"}" \
  "$BASE_URL/api/v1/downloads"
