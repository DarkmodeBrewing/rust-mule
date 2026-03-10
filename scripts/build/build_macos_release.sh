#!/usr/bin/env bash
set -euo pipefail

BIN="rust-mule"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found in PATH" >&2
  exit 1
fi

git_sha() {
  git rev-parse --short HEAD 2>/dev/null || echo "nogit"
}

default_target() {
  case "$(uname -m 2>/dev/null || echo unknown)" in
    arm64|aarch64) echo "aarch64-apple-darwin" ;;
    x86_64) echo "x86_64-apple-darwin" ;;
    *) echo "unsupported" ;;
  esac
}

target_arch_label() {
  case "${1}" in
    aarch64-apple-darwin) echo "arm64" ;;
    x86_64-apple-darwin) echo "x86_64" ;;
    *) echo "unknown" ;;
  esac
}

main() {
  local rust_target
  local target_bin
  local target_arch
  local out_root
  local out_dir
  local tar_path
  local deployment_target

  rust_target="${MACOS_BUILD_TARGET:-$(default_target)}"
  target_arch="$(target_arch_label "${rust_target}")"

  if [[ "${rust_target}" == "unsupported" || "${target_arch}" == "unknown" ]]; then
    echo "Unsupported macOS target: ${rust_target}" >&2
    exit 1
  fi

  if [[ "${rust_target}" == "x86_64-apple-darwin" ]]; then
    deployment_target="${MACOSX_DEPLOYMENT_TARGET:-12.0}"
    export MACOSX_DEPLOYMENT_TARGET="${deployment_target}"
  else
    deployment_target="${MACOSX_DEPLOYMENT_TARGET:-}"
  fi

  cargo build --release --locked --target "${rust_target}" --bin "${BIN}"

  target_bin="target/${rust_target}/release/${BIN}"

  if [[ ! -f "${target_bin}" ]]; then
    echo "Expected ${target_bin} to exist after build" >&2
    exit 1
  fi

  # Best-effort strip for macOS toolchain.
  strip -x "${target_bin}" 2>/dev/null || true

  out_root="dist"
  out_dir="${out_root}/${BIN}-$(git_sha)-macos-${target_arch}"
  mkdir -p "${out_dir}"

  cp "${target_bin}" "${out_dir}/${BIN}"
  cp "config.toml" "${out_dir}/config.example.toml"

  cat >"${out_dir}/README.txt" <<EOT
rust-mule macOS release bundle

Run:
  ./rust-mule

Platform:
  Target triple: ${rust_target}
  CPU architecture: ${target_arch}
EOT

  if [[ -n "${deployment_target}" ]]; then
    cat >>"${out_dir}/README.txt" <<EOT
  Built with MACOSX_DEPLOYMENT_TARGET=${deployment_target}
EOT
  else
    cat >>"${out_dir}/README.txt" <<'EOT'
  Built without an explicit MACOSX_DEPLOYMENT_TARGET override
EOT
  fi

  cat >>"${out_dir}/README.txt" <<'EOT'

Config:
  rust-mule reads ./config.toml from the current working directory.
  Copy config.example.toml -> config.toml and edit as needed.

Data:
  Runtime state is written under [general].data_dir (default: data/).
EOT

  tar_path="${out_dir}.tar.gz"
  tar -czf "${tar_path}" -C "${out_root}" "$(basename "${out_dir}")"
  echo "Wrote ${tar_path}"
}

main "$@"
