#!/usr/bin/env bash
set -euo pipefail
SCENARIO=integrity exec "$(dirname "$0")/download_soak_bg.sh" "$@"
