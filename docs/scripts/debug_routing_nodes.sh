#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: docs/scripts/debug_routing_nodes.sh --bucket N [--base-url URL] [--token TOKEN] [--token-file PATH]

Calls:
  GET /api/v1/debug/routing/nodes?bucket=N

Options:
  --bucket N           Required bucket index (0..127)
  --base-url URL       Default: http://127.0.0.1:17835
  --token TOKEN        Bearer token (overrides --token-file)
  --token-file PATH    Default: data/api.token
USAGE
}

BASE_URL="http://127.0.0.1:17835"
TOKEN_FILE="data/api.token"
TOKEN=""
BUCKET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bucket) BUCKET="$2"; shift 2 ;;
    --base-url) BASE_URL="$2"; shift 2 ;;
    --token) TOKEN="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$BUCKET" ]]; then
  echo "--bucket is required" >&2
  usage
  exit 2
fi

if [[ -z "$TOKEN" ]]; then
  TOKEN="$(cat "$TOKEN_FILE")"
fi

curl -sS \
  -H "Authorization: Bearer $TOKEN" \
  "$BASE_URL/api/v1/debug/routing/nodes?bucket=$BUCKET"
