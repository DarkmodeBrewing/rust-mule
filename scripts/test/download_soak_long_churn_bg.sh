#!/usr/bin/env bash
set -euo pipefail
SCENARIO=long_churn exec "$(dirname "$0")/download_soak_bg.sh" "$@"
