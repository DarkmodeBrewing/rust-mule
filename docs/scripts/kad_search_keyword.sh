#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/kad_search_keyword.sh (--query "words..." | --keyword-id-hex HEX) [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  POST /api/v1/kad/search_keyword

Options:
  --query TEXT         Search query (iMule-style: we hash the first extracted word)
  --keyword-id-hex HEX 32 hex chars (16 bytes). Bypasses tokenization/hashing.
  --base-url URL       Default: http://127.0.0.1:17835
  --token TOKEN        Bearer token (overrides --token-file)
  --token-file PATH    Default: data/api.token
EOF
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
QUERY=""
KEYWORD_ID_HEX=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --query) QUERY="$2"; shift 2 ;;
    --keyword-id-hex) KEYWORD_ID_HEX="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$QUERY" && -z "$KEYWORD_ID_HEX" ]]; then
  echo "Missing --query or --keyword-id-hex" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

if [[ -n "$KEYWORD_ID_HEX" ]]; then
  BODY="{\"keyword_id_hex\":\"$KEYWORD_ID_HEX\"}"
else
  BODY="{\"query\":\"$QUERY\"}"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$BODY" \
  "$BASE_URL/api/v1/kad/search_keyword"
