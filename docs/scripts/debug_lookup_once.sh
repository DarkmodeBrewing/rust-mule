#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: docs/scripts/debug_lookup_once.sh [--target-id-hex HEX] [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  POST /debug/lookup_once

Options:
  --target-id-hex HEX  Optional 16-byte hex KadID (32 hex chars)
  --base-url URL       Default: http://127.0.0.1:17835
  --token TOKEN        Bearer token (overrides --token-file)
  --token-file PATH    Default: data/api.token
USAGE
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target-id-hex) TARGET="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

if [[ -n "$TARGET" ]]; then
  BODY="{\"target_id_hex\":\"$TARGET\"}"
else
  BODY="{}"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$BODY" \
  "$BASE_URL/debug/lookup_once"
