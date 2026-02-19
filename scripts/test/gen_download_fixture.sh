#!/usr/bin/env bash
set -euo pipefail

# Generate download fixture JSON entries from one or more local files.
#
# Usage:
#   scripts/test/gen_download_fixture.sh /path/to/file1 [/path/to/file2 ...]
#   scripts/test/gen_download_fixture.sh --out scripts/test/my_fixtures.json /path/to/file1
#   scripts/test/gen_download_fixture.sh --publish --base-url http://127.0.0.1:17835 /path/to/file1
#
# Notes:
# - Uses rust-mule's built-in MD4 implementation via `download_fixture_gen`.
# - Output schema matches DOWNLOAD_FIXTURES_FILE expectations.
# - Optional publish step calls scripts/docs/kad_publish_source.sh for each generated fixture.

OUT_FILE=""
PUBLISH="0"
BASE_URL="http://127.0.0.1:17835"
TOKEN=""
TOKEN_FILE="data/api.token"
PUBLISH_SCRIPT=""
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
  --publish)
    PUBLISH="1"
    shift
    ;;
  --base-url)
    shift
    if (($# == 0)); then
      echo "ERROR: --base-url requires a URL" >&2
      exit 2
    fi
    BASE_URL="$1"
    shift
    ;;
  --token)
    shift
    if (($# == 0)); then
      echo "ERROR: --token requires a value" >&2
      exit 2
    fi
    TOKEN="$1"
    shift
    ;;
  --token-file)
    shift
    if (($# == 0)); then
      echo "ERROR: --token-file requires a path" >&2
      exit 2
    fi
    TOKEN_FILE="$1"
    shift
    ;;
  --publish-script)
    shift
    if (($# == 0)); then
      echo "ERROR: --publish-script requires a path" >&2
      exit 2
    fi
    PUBLISH_SCRIPT="$1"
    shift
    ;;
  -h | --help)
    cat <<'EOF'
usage: scripts/test/gen_download_fixture.sh [--out <json_file>] [--publish] [--base-url <url>] [--token <token> | --token-file <path>] [--publish-script <path>] <file> [<file>...]

Generates JSON fixture entries:
  [
    {"file_name":"...","file_size":123,"file_hash_md4_hex":"..."},
    ...
  ]

Options:
  --out <json_file>      Write generated fixture JSON to file.
  --publish              Publish each generated fixture hash to KAD source store.
  --base-url <url>       API base URL for publish step (default: http://127.0.0.1:17835).
  --token <token>        API bearer token (overrides --token-file).
  --token-file <path>    Token file for publish step (default: data/api.token).
  --publish-script <p>   Publish helper script path (default: scripts/docs/kad_publish_source.sh).
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
if [[ -z "$PUBLISH_SCRIPT" ]]; then
  PUBLISH_SCRIPT="$ROOT_DIR/scripts/docs/kad_publish_source.sh"
fi

FIXTURE_JSON="$(cd "$ROOT_DIR" && cargo run --quiet --bin download_fixture_gen -- "${ARGS[@]}")"

if [[ -n "$OUT_FILE" ]]; then
  mkdir -p "$(dirname "$OUT_FILE")"
  printf '%s\n' "$FIXTURE_JSON" >"$OUT_FILE"
  echo "wrote fixture file: $OUT_FILE"
else
  printf '%s\n' "$FIXTURE_JSON"
fi

if [[ "$PUBLISH" == "1" ]]; then
  command -v jq >/dev/null 2>&1 || {
    echo "ERROR: jq is required for --publish" >&2
    exit 1
  }
  [[ -f "$PUBLISH_SCRIPT" ]] || {
    echo "ERROR: publish script not found: $PUBLISH_SCRIPT" >&2
    exit 1
  }

  AUTH_ARGS=()
  if [[ -n "$TOKEN" ]]; then
    AUTH_ARGS+=(--token "$TOKEN")
  else
    AUTH_ARGS+=(--token-file "$TOKEN_FILE")
  fi

  echo "publishing generated fixtures via $PUBLISH_SCRIPT to $BASE_URL"
  printf '%s\n' "$FIXTURE_JSON" \
    | jq -c '.[]' \
    | while IFS= read -r row; do
      file_id_hex="$(printf '%s\n' "$row" | jq -r '.file_hash_md4_hex')"
      file_size="$(printf '%s\n' "$row" | jq -r '.file_size')"
      file_name="$(printf '%s\n' "$row" | jq -r '.file_name')"
      echo "publish file_name=$file_name file_id_hex=$file_id_hex file_size=$file_size"
      "$PUBLISH_SCRIPT" \
        --base-url "$BASE_URL" \
        "${AUTH_ARGS[@]}" \
        --file-id-hex "$file_id_hex" \
        --file-size "$file_size" >/dev/null
    done
  echo "publish complete"
fi
