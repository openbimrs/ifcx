#!/usr/bin/env bash
# Complete standalone verification gate for openbimrs/ifcx.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
# Default-off and fail-closed paths: no integrity hashing, no filesystem resolver.
cargo test -p openbim-ifcx --no-default-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
