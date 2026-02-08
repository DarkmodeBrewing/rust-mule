#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/kad_keyword_results_get.sh --keyword-id-hex HEX [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  GET /kad/keyword_results/:keyword_id_hex

Options:
  --keyword-id-hex HEX  32 hex chars (16 bytes)
  --base-url URL        Default: http://127.0.0.1:17835
  --token TOKEN         Bearer token (overrides --token-file)
  --token-file PATH     Default: data/api.token
EOF
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
KEYWORD_ID_HEX=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --keyword-id-hex) KEYWORD_ID_HEX="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$KEYWORD_ID_HEX" ]]; then
  echo "Missing --keyword-id-hex" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

curl -sS -H "Authorization: Bearer $TOKEN" "$BASE_URL/kad/keyword_results/$KEYWORD_ID_HEX"

