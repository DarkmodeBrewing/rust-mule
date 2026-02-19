#!/usr/bin/env bash
set -euo pipefail
SCENARIO=concurrency exec "$(dirname "$0")/download_soak_bg.sh" "$@"
