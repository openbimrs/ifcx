#!/usr/bin/env bash
# Complete standalone verification gate for openbimrs/ifcx.
#
# Usage: scripts/gate.sh [section...]
#
# With no argument every section runs, in order: this is the full gate, and
# the one to run before a merge. CI runs each section as its own parallel job
# and passes only when all of them pass, so the union is identical:
#
#   rust      formatting, check, tests, clippy, rustdoc, generated docs,
#             the docs site build, packaging
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

    # Documentation. Every generated docs file and region (crate reference
    # pages, the assembled changelog, the install table, the ADR index, the
    # upstream evidence, the contributing pages, the README crate table)
    # must equal what `cargo run -p xtask -- docs` writes, so changing a
    # manifest, README, changelog, ADR or binding surface without
    # regenerating fails here. Reads the Python API with python3's `ast`.
    cargo run --quiet --locked -p xtask -- docs --check
    # Every unfinished-work marker names its issue, as `TODO(#N)`.
    cargo run --quiet --locked -p xtask -- todo --check
    # No buildingSMART file or text is committed (scripts/check-leakage.py;
    # the Pages workflow runs it on the built site against upstream).
    python3 scripts/check-leakage.py
    # The site build resolves every link and checks the diagrams. It needs
    # the docs toolchain (`npm ci` at the root, which CI runs); without
    # node_modules it is skipped so the gate still runs without Node.
    if [[ -d node_modules ]]; then
        local log
        log=$(mktemp)
        npm run --silent docs:build >"$log" 2>&1 \
            || { echo "docs build failed:" >&2; tail -30 "$log" >&2; rm -f "$log"; exit 1; }
        rm -f "$log"
        echo "docs build ok"
    else
        echo "warning: no node_modules; docs site build NOT run (npm ci first)" >&2
    fi
    # Every publishable crate must package and build from its .crate alone.
    cargo package --locked -p openbim-ifcx
    # Cargo 1.88 resolves a packaged crate's dependencies from crates.io
    # only, so geometry cannot be packaged while it requires an
    # openbim-ifcx version that is not released yet (the window between a
    # version bump and the openbim-ifcx release). Release openbim-ifcx
    # first; `cargo publish` in the release job then verifies geometry.
    local core
    core=$(cargo metadata --no-deps --format-version 1 \
        | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="openbim-ifcx"))')
    if curl -fsS --max-time 30 https://index.crates.io/op/en/openbim-ifcx | grep -q "\"vers\":\"$core\""; then
        cargo package --locked -p openbim-ifcx-geometry
    else
        echo "warning: openbim-ifcx $core is not on crates.io yet; skipping cargo package of openbim-ifcx-geometry (release openbim-ifcx first)" >&2
    fi
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
