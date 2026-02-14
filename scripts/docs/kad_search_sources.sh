#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/docs/kad_search_sources.sh --file-id-hex HEX [--file-size N] [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  POST /api/v1/kad/search_sources

Options:
  --file-id-hex HEX    32 hex chars (16 bytes)
  --file-size N        Default: 0
  --base-url URL       Default: http://127.0.0.1:17835
  --token TOKEN        Bearer token (overrides --token-file)
  --token-file PATH    Default: data/api.token
EOF
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
FILE_ID_HEX=""
FILE_SIZE="0"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file-id-hex) FILE_ID_HEX="$2"; shift 2 ;;
    --file-size) FILE_SIZE="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$FILE_ID_HEX" ]]; then
  echo "Missing --file-id-hex" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"file_id_hex\":\"$FILE_ID_HEX\",\"file_size\":$FILE_SIZE}" \
  "$BASE_URL/api/v1/kad/search_sources"

