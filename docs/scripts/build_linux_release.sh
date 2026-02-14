#!/usr/bin/env bash
set -euo pipefail

# Backward-compatible wrapper. Use scripts/build/build_linux_release.sh going forward.
exec scripts/build/build_linux_release.sh "$@"
