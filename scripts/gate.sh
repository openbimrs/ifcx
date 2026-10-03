#!/usr/bin/env bash
# Complete standalone verification gate for openbimrs/ifcx.
#
# Usage: scripts/gate.sh [section...]
#
# With no argument every section runs, in order: this is the full gate, and
# the one to run before a merge. CI runs each section as its own parallel job
# and passes only when all of them pass, so the union is identical:
#
#   rust      formatting, check, tests, clippy, rustdoc, packaging
#   bindings  JavaScript and Python bindings, built and tested as shipped
set -euo pipefail

cd "$(dirname "$0")/.."

gate_rust() {
    cargo fmt --all -- --check
    cargo check --workspace --all-targets --all-features
    cargo test --workspace --all-features
    # Default-off and fail-closed paths: no integrity hashing, no filesystem resolver.
    cargo test -p openbim-ifcx --no-default-features
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
    # Every publishable crate must package and build from its .crate alone.
    cargo package --locked -p openbim-ifcx
    cargo package --locked -p openbim-ifcx-geometry
}

gate_bindings() {
    # The wasm crate is empty natively, so clippy sees its code only for
    # wasm32 (rust-toolchain.toml installs the target).
    cargo clippy -p openbim-ifcx-wasm --target wasm32-unknown-unknown --locked -- -D warnings

    # JavaScript (@openbim/ifcx): build the wasm module with the pinned
    # wasm-bindgen CLI for Node, bundlers and plain browser pages, run the
    # Node suite against the built package, then pack it and check every
    # target as installed, including both browser builds in headless Chrome
    # (IFCX_SKIP_BROWSER=1 skips only those), so the binding is proven to
    # work from JS, not just to compile.
    if [[ -n "${IFCX_SKIP_JS:-}" ]]; then
        echo "warning: IFCX_SKIP_JS set; JS binding suite NOT run" >&2
    else
        crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh
    fi

    # Python (openbim-ifcx): build the abi3 wheel with maturin, install it
    # into a throwaway uv venv, and run the Python suite against it.
    if [[ -n "${IFCX_SKIP_PYTHON:-}" ]]; then
        echo "warning: IFCX_SKIP_PYTHON set; Python binding suite NOT run" >&2
    else
        crates/openbim-ifcx-py/scripts/check-python.sh
    fi
}

sections=("$@")
if [[ ${#sections[@]} -eq 0 ]]; then
    sections=(rust bindings)
fi
for section in "${sections[@]}"; do
    case "$section" in
        rust | bindings) ;;
        *) echo "error: unknown gate section '$section' (rust, bindings)" >&2; exit 2 ;;
    esac
done
for section in "${sections[@]}"; do
    echo "== gate: $section"
    "gate_$section"
done
