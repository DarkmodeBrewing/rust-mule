#!/usr/bin/env bash
set -euo pipefail

# Requires: cargo-llvm-cov (install with: cargo install cargo-llvm-cov)
MIN_LINES_COVERAGE="${MIN_LINES_COVERAGE:-35}"

cargo llvm-cov --workspace --all-features --fail-under-lines "${MIN_LINES_COVERAGE}"
