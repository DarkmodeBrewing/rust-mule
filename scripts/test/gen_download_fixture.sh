#!/usr/bin/env bash
set -euo pipefail

# Generate download fixture JSON entries from one or more local files.
#
# Usage:
#   scripts/test/gen_download_fixture.sh /path/to/file1 [/path/to/file2 ...]
#   scripts/test/gen_download_fixture.sh --out scripts/test/my_fixtures.json /path/to/file1
#
# Notes:
# - Uses rust-mule's built-in MD4 implementation via `download_fixture_gen`.
# - Output schema matches DOWNLOAD_FIXTURES_FILE expectations.

OUT_FILE=""
ARGS=()

while (($# > 0)); do
  case "$1" in
  --out)
    shift
    if (($# == 0)); then
      echo "ERROR: --out requires a path" >&2
      exit 2
    fi
    OUT_FILE="$1"
    shift
    ;;
  -h | --help)
    cat <<'EOF'
usage: scripts/test/gen_download_fixture.sh [--out <json_file>] <file> [<file>...]

Generates JSON fixture entries:
  [
    {"file_name":"...","file_size":123,"file_hash_md4_hex":"..."},
    ...
  ]
EOF
    exit 0
    ;;
  *)
    ARGS+=("$1")
    shift
    ;;
  esac
done

if ((${#ARGS[@]} == 0)); then
  echo "ERROR: at least one input file is required" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ -n "$OUT_FILE" ]]; then
  mkdir -p "$(dirname "$OUT_FILE")"
  (cd "$ROOT_DIR" && cargo run --quiet --bin download_fixture_gen -- "${ARGS[@]}") >"$OUT_FILE"
  echo "wrote fixture file: $OUT_FILE"
else
  (cd "$ROOT_DIR" && cargo run --quiet --bin download_fixture_gen -- "${ARGS[@]}")
fi
