#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/kad_publish_keyword.sh (--query "words..." | --keyword-id-hex HEX) --file-id-hex HEX --filename NAME --file-size N [--file-type TYPE] [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  POST /api/v1/kad/publish_keyword

Options:
  --query TEXT          Keyword query (iMule-style: we hash the first extracted word)
  --keyword-id-hex HEX  32 hex chars (16 bytes). Bypasses tokenization/hashing.
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
KEYWORD_ID_HEX=""
FILE_ID_HEX=""
FILENAME=""
FILE_SIZE=""
FILE_TYPE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --query) QUERY="$2"; shift 2 ;;
    --keyword-id-hex) KEYWORD_ID_HEX="$2"; shift 2 ;;
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

if [[ -z "$FILE_ID_HEX" || -z "$FILENAME" || -z "$FILE_SIZE" ]]; then
  echo "Missing required args" >&2
  usage
  exit 2
fi

if [[ -z "$QUERY" && -z "$KEYWORD_ID_HEX" ]]; then
  echo "Missing --query or --keyword-id-hex" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

if [[ -n "$KEYWORD_ID_HEX" ]]; then
  PREFIX="{\"keyword_id_hex\":\"$KEYWORD_ID_HEX\""
else
  PREFIX="{\"query\":\"$QUERY\""
fi

if [[ -n "$FILE_TYPE" ]]; then
  BODY="${PREFIX},\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE,\"file_type\":\"$FILE_TYPE\"}"
else
  BODY="${PREFIX},\"file_id_hex\":\"$FILE_ID_HEX\",\"filename\":\"$FILENAME\",\"file_size\":$FILE_SIZE}"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$BODY" \
  "$BASE_URL/api/v1/kad/publish_keyword"
