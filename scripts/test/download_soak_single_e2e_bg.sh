#!/usr/bin/env bash
set -euo pipefail
SCENARIO=single_e2e exec "$(dirname "$0")/download_soak_bg.sh" "$@"
