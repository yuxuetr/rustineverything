#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/Users/hal/.target}"

cargo test --features server -p rustineverything-module-cases
cargo test --features server -p rustineverything-module-search
