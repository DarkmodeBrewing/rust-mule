#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:17835}"
TOKEN_FILE="${TOKEN_FILE:-data/api.token}"
DURATION_SECS="${DURATION_SECS:-21600}"
INTERVAL_SECS="${INTERVAL_SECS:-5}"
OUT_FILE="${OUT_FILE:-/tmp/rust-mule-kad-phase0-longrun-$(date +%Y%m%d_%H%M%S).tsv}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
baseline_script="$script_dir/kad_phase0_baseline.sh"

if [[ ! -x "$baseline_script" ]]; then
  echo "missing or non-executable baseline script: $baseline_script" >&2
  exit 1
fi

echo "starting long-run baseline capture:"
echo "  base_url=$BASE_URL"
echo "  token_file=$TOKEN_FILE"
echo "  duration_secs=$DURATION_SECS"
echo "  interval_secs=$INTERVAL_SECS"
echo "  out_file=$OUT_FILE"

BASE_URL="$BASE_URL" \
TOKEN_FILE="$TOKEN_FILE" \
DURATION_SECS="$DURATION_SECS" \
INTERVAL_SECS="$INTERVAL_SECS" \
OUT_FILE="$OUT_FILE" \
  "$baseline_script"

restart_markers="$(
  awk -F '\t' '
    NR == 1 { for (i=1; i<=NF; i++) h[$i]=i; next }
    { m += $(h["restart_marker"]); }
    END { print m + 0 }
  ' "$OUT_FILE"
)"
sam_desync_total_max="$(
  awk -F '\t' '
    NR == 1 { for (i=1; i<=NF; i++) h[$i]=i; next }
    { v = $(h["sam_framing_desync_total"]) + 0; if (v > max) max = v; }
    END { print max + 0 }
  ' "$OUT_FILE"
)"
dropped_legacy_kad1_total_max="$(
  awk -F '\t' '
    NR == 1 { for (i=1; i<=NF; i++) h[$i]=i; next }
    { v = $(h["dropped_legacy_kad1_total"]) + 0; if (v > max) max = v; }
    END { print max + 0 }
  ' "$OUT_FILE"
)"
dropped_unhandled_opcode_total_max="$(
  awk -F '\t' '
    NR == 1 { for (i=1; i<=NF; i++) h[$i]=i; next }
    { v = $(h["dropped_unhandled_opcode_total"]) + 0; if (v > max) max = v; }
    END { print max + 0 }
  ' "$OUT_FILE"
)"

echo "long-run summary: restart_markers=$restart_markers sam_framing_desync_total_max=$sam_desync_total_max dropped_legacy_kad1_total_max=$dropped_legacy_kad1_total_max dropped_unhandled_opcode_total_max=$dropped_unhandled_opcode_total_max"
echo "long-run baseline done: $OUT_FILE"
