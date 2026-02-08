#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: docs/scripts/health.sh [--base-url URL]

Calls:
  GET /health   (no auth)

Options:
  --base-url URL   Default: http://127.0.0.1:17835
EOF
}

BASE_URL="http://127.0.0.1:17835"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url) BASE_URL="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

curl -sS "$BASE_URL/health"

