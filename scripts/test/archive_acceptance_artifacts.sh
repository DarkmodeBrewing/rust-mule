#!/usr/bin/env bash
set -euo pipefail

# Archive high-value outputs from a download phase0 acceptance run.
#
# Defaults:
# - selects latest /tmp/rust-mule-download-phase0-accept-*
# - writes under artifacts/soak/<run_basename>
#
# Usage:
#   scripts/test/archive_acceptance_artifacts.sh
#   scripts/test/archive_acceptance_artifacts.sh --run-dir /tmp/rust-mule-download-phase0-accept-20260307_145056
#   scripts/test/archive_acceptance_artifacts.sh --dest-root artifacts/soak
#   scripts/test/archive_acceptance_artifacts.sh --stack-bundle /tmp/rust-mule-download-stack-20260307_175944.tar.gz

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

RUN_DIR=""
DEST_ROOT="$ROOT/artifacts/soak"
STACK_BUNDLE=""

usage() {
  cat <<'USAGE'
usage: scripts/test/archive_acceptance_artifacts.sh [--run-dir <path>] [--dest-root <path>] [--stack-bundle <path>]
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
  --run-dir)
    RUN_DIR="${2:-}"
    shift 2
    ;;
  --dest-root)
    DEST_ROOT="${2:-}"
    shift 2
    ;;
  --stack-bundle)
    STACK_BUNDLE="${2:-}"
    shift 2
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "ERROR: unknown arg: $1" >&2
    usage >&2
    exit 2
    ;;
  esac
done

if [[ -z "$RUN_DIR" ]]; then
  RUN_DIR="$(ls -dt /tmp/rust-mule-download-phase0-accept-* 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "$RUN_DIR" || ! -d "$RUN_DIR" ]]; then
  echo "ERROR: run dir not found: $RUN_DIR" >&2
  exit 1
fi

RUN_BASENAME="$(basename "$RUN_DIR")"
DEST_DIR="$DEST_ROOT/$RUN_BASENAME"

mkdir -p "$DEST_DIR"
mkdir -p "$DEST_DIR/kad-gate" "$DEST_DIR/resume-soak" "$DEST_DIR/snapshots"

copy_if_exists() {
  local src="$1"
  local dst="$2"
  if [[ -f "$src" ]]; then
    cp -f "$src" "$dst"
  fi
}

# Top-level
copy_if_exists "$RUN_DIR/summary.txt" "$DEST_DIR/summary.txt"

# KAD gate files
copy_if_exists "$RUN_DIR/kad-gate/gate.tsv" "$DEST_DIR/kad-gate/gate.tsv"
copy_if_exists "$RUN_DIR/kad-gate/compare.tsv" "$DEST_DIR/kad-gate/compare.tsv"
copy_if_exists "$RUN_DIR/kad-gate/before.tsv" "$DEST_DIR/kad-gate/before.tsv"
copy_if_exists "$RUN_DIR/kad-gate/after.tsv" "$DEST_DIR/kad-gate/after.tsv"

# Resume soak report and diagnostics
copy_if_exists "$RUN_DIR/resume-soak/resume_report.txt" "$DEST_DIR/resume-soak/resume_report.txt"
for f in "$RUN_DIR"/resume-soak/*_diag.json "$RUN_DIR"/resume-soak/*_timeout_*.json; do
  [[ -f "$f" ]] || continue
  cp -f "$f" "$DEST_DIR/resume-soak/"
done

# Snapshot state
for f in "$RUN_DIR"/snapshots/*.json; do
  [[ -f "$f" ]] || continue
  cp -f "$f" "$DEST_DIR/snapshots/"
done

# Optional stack bundle attachment
if [[ -n "$STACK_BUNDLE" ]]; then
  if [[ -f "$STACK_BUNDLE" ]]; then
    cp -f "$STACK_BUNDLE" "$DEST_DIR/"
  else
    echo "WARN: stack bundle not found: $STACK_BUNDLE" >&2
  fi
fi

{
  echo "run_dir=$RUN_DIR"
  echo "archived_at_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "dest_dir=$DEST_DIR"
  if [[ -n "$STACK_BUNDLE" ]]; then
    echo "stack_bundle=$STACK_BUNDLE"
  fi
} >"$DEST_DIR/archive_manifest.txt"

echo "archived acceptance artifacts:"
echo "  run_dir=$RUN_DIR"
echo "  dest_dir=$DEST_DIR"
