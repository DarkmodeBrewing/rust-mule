#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/kad_search_keyword.sh --query "words..." [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  POST /kad/search_keyword

Options:
  --query TEXT         Search query (iMule-style: we hash the first extracted word)
  --base-url URL       Default: http://127.0.0.1:17835
  --token TOKEN        Bearer token (overrides --token-file)
  --token-file PATH    Default: data/api.token
EOF
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
QUERY=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --query) QUERY="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$QUERY" ]]; then
  echo "Missing --query" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$QUERY\"}" \
  "$BASE_URL/kad/search_keyword"

