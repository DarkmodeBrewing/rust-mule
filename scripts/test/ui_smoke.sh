#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
UI_DIR="$ROOT_DIR/ui"
NVM_SH="${NVM_SH:-$HOME/.nvm/nvm.sh}"
PLAYWRIGHT_BROWSER="${UI_BROWSER:-chromium}"

if [[ ! -s "$NVM_SH" ]]; then
  echo "ERROR: nvm init script not found at $NVM_SH" >&2
  exit 1
fi

# shellcheck source=/dev/null
. "$NVM_SH"

if ! command -v npm >/dev/null 2>&1; then
  echo "ERROR: npm not available after sourcing $NVM_SH" >&2
  exit 1
fi

cd "$UI_DIR"

if [[ ! -d node_modules ]]; then
  echo "ERROR: ui/node_modules is missing; run 'cd ui && npm install' first" >&2
  exit 1
fi

set +e
OUTPUT="$(
  CI=1 \
  UI_BROWSER="$PLAYWRIGHT_BROWSER" \
  npm run test:ui:smoke 2>&1
)"
STATUS=$?
set -e

printf '%s\n' "$OUTPUT"

if [[ $STATUS -ne 0 ]] && grep -q "Host system is missing dependencies to run browsers" <<<"$OUTPUT"; then
  cat >&2 <<EOF
UI smoke failed because Playwright's browser runtime dependencies are missing.

To install them on Debian/Ubuntu:
  sudo npx playwright install-deps

Or install the packages Playwright listed in the error output, then rerun:
  bash scripts/test/ui_smoke.sh

Notes:
  - This runner already sources nvm from: $NVM_SH
  - Playwright is configured headless
  - Chromium sandbox is disabled for container/CI friendliness
EOF
fi

exit $STATUS
