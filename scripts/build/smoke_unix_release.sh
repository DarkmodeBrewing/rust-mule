#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <archive.tar.gz>" >&2
  exit 1
fi

ARCHIVE="$1"

if [[ ! -f "${ARCHIVE}" ]]; then
  echo "archive not found: ${ARCHIVE}" >&2
  exit 1
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

tar -xzf "${ARCHIVE}" -C "${TMP_DIR}"

EXTRACT_ROOT="$(find "${TMP_DIR}" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
if [[ -z "${EXTRACT_ROOT}" ]]; then
  echo "expected extracted release directory in ${ARCHIVE}" >&2
  exit 1
fi

BIN="${EXTRACT_ROOT}/rust-mule"
CFG="${EXTRACT_ROOT}/config.example.toml"

if [[ ! -x "${BIN}" ]]; then
  echo "expected executable ${BIN}" >&2
  exit 1
fi

if [[ ! -f "${CFG}" ]]; then
  echo "expected config example ${CFG}" >&2
  exit 1
fi

"${BIN}" --version >/dev/null
"${BIN}" --help >/dev/null
"${BIN}" --check-config --config "${CFG}" >/dev/null

echo "smoke OK: ${ARCHIVE}"
