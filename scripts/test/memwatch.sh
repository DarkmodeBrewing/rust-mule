#!/usr/bin/env bash
set -euo pipefail

# --------------------------------------------
# memwatch.sh — track Linux process memory over time
# --------------------------------------------

INTERVAL=60
OUTPUT_FILE=""
PID=""

usage() {
  cat <<EOF
Usage:
  $0 <pid> --output-file <path> [--interval <seconds>]

Options:
  --output-file <path>   Base output file (timestamp will be added)
  --interval <seconds>   Poll interval in seconds (default: 60)
  -h, --help             Show this help
EOF
}

# -----------------------------
# Argument parsing
# -----------------------------
if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

PID="$1"
shift

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-file)
      OUTPUT_FILE="$2"
      shift 2
      ;;
    --interval)
      INTERVAL="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$OUTPUT_FILE" ]]; then
  echo "Error: --output-file is required" >&2
  exit 1
fi

if [[ ! -d "/proc/$PID" ]]; then
  echo "Error: PID $PID does not exist" >&2
  exit 1
fi

ROLLUP="/proc/$PID/smaps_rollup"

if [[ ! -r "$ROLLUP" ]]; then
  echo "Error: cannot read $ROLLUP (need sufficient permissions)" >&2
  exit 1
fi

# -----------------------------
# Timestamp output filename
# -----------------------------
TIMESTAMP=$(date +"%Y-%m-%dT%H-%M-%S")

DIR=$(dirname "$OUTPUT_FILE")
BASE=$(basename "$OUTPUT_FILE")

EXT=""
NAME="$BASE"

if [[ "$BASE" == *.* ]]; then
  EXT=".${BASE##*.}"
  NAME="${BASE%.*}"
fi

OUTPUT_FILE="${DIR}/${NAME}.${TIMESTAMP}${EXT}"

# -----------------------------
# Init output file
# -----------------------------
echo -e "timestamp\tpid\tpss_kb\tprivate_dirty_kb\tanon_kb\tfile_kb\trss_kb" > "$OUTPUT_FILE"

echo "Tracking memory for PID $PID"
echo "Writing to $OUTPUT_FILE"
echo "Polling every ${INTERVAL}s"
echo "Press Ctrl-C to stop"
echo

# -----------------------------
# Graceful shutdown
# -----------------------------
trap 'echo -e "\nStopped."; exit 0' SIGINT SIGTERM

# -----------------------------
# Main loop
# -----------------------------
while true; do
  if [[ ! -d "/proc/$PID" ]]; then
    echo "Process $PID exited, stopping."
    exit 0
  fi

  NOW=$(date -Is)

  PSS=$(awk '/^Pss:/ {print $2}' "$ROLLUP")
  PRIVATE_DIRTY=$(awk '/^Private_Dirty:/ {print $2}' "$ROLLUP")
  ANON=$(awk '/^Pss_Anon:/ {print $2}' "$ROLLUP")
  FILE=$(awk '/^Pss_File:/ {print $2}' "$ROLLUP")
  RSS=$(awk '/^Rss:/ {print $2}' "$ROLLUP")

  echo -e "${NOW}\t${PID}\t${PSS}\t${PRIVATE_DIRTY}\t${ANON}\t${FILE}\t${RSS}" >> "$OUTPUT_FILE"

  sleep "$INTERVAL"
done
