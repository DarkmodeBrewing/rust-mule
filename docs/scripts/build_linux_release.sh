#!/usr/bin/env bash
set -euo pipefail

BIN="rust-mule"
TARGET_BIN="target/release/${BIN}"

git_sha() {
  git rev-parse --short HEAD 2>/dev/null || echo "nogit"
}

arch() {
  uname -m 2>/dev/null || echo "unknown"
}

main() {
  cargo build --release --locked --bin "${BIN}"

  if [[ ! -f "${TARGET_BIN}" ]]; then
    echo "Expected ${TARGET_BIN} to exist after build" >&2
    exit 1
  fi

  # Best-effort strip to reduce size.
  strip "${TARGET_BIN}" 2>/dev/null || true

  OUT_ROOT="dist"
  OUT_DIR="${OUT_ROOT}/${BIN}-$(git_sha)-linux-$(arch)"
  mkdir -p "${OUT_DIR}"

  cp "${TARGET_BIN}" "${OUT_DIR}/${BIN}"
  cp "config.toml" "${OUT_DIR}/config.example.toml"

  # Convenience: include minimal run instructions next to the artifact.
  cat >"${OUT_DIR}/README.txt" <<'EOF'
rust-mule Linux release bundle

Run:
  ./rust-mule

Config:
  rust-mule reads ./config.toml from the current working directory.
  Copy config.example.toml -> config.toml and edit as needed.

Data:
  Runtime state is written under [general].data_dir (default: data/).
EOF

  TAR="${OUT_DIR}.tar.gz"
  tar -czf "${TAR}" -C "${OUT_ROOT}" "$(basename "${OUT_DIR}")"
  echo "Wrote ${TAR}"
}

main "$@"

