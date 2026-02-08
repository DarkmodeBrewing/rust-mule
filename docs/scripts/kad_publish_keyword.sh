#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/kad_publish_keyword.sh --query "words..." --file-id-hex HEX --filename NAME --file-size N [--file-type TYPE] [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  POST /kad/publish_keyword

Options:
  --query TEXT          Keyword query (iMule-style: we hash the first extracted word)
  --file-id-hex HEX     32 hex chars (16 bytes)
  --filename NAME       Human filename to publish
  --file-size N         Size in bytes
  --file-type TYPE      Optional file type string (e.g. Audio/Video/Image/Doc/Pro)
  --base-url URL        Default: http://127.0.0.1:17835
  --token TOKEN         Bearer token (overrides --token-file)
  --token-file PATH     Default: data/api.token
EOF
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
QUERY=""
FILE_ID_HEX=""
FILENAME=""
FILE_SIZE=""
FILE_TYPE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --query) QUERY="$2"; shift 2 ;;
    --file-id-hex) FILE_ID_HEX="$2"; shift 2 ;;
    --filename) FILENAME="$2"; shift 2 ;;
    --file-size) FILE_SIZE="$2"; shift 2 ;;
    --file-type) FILE_TYPE="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$QUERY" || -z "$FILE_ID_HEX" || -z "$FILENAME" || -z "$FILE_SIZE" ]]; then
  echo "Missing required args" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

if [[ -n "$FILE_TYPE" ]]; then
  BODY="{\"query\":\"$QUERY\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE,\"file_type\":\"$FILE_TYPE\"}"
else
  BODY="{\"query\":\"$QUERY\",\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE}"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$BODY" \
  "$BASE_URL/kad/publish_keyword"

