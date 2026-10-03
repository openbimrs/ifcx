#!/usr/bin/env bash
# Build the openbim-ifcx-wasm Node package, then run its JS suite against it.
#
#   crates/openbim-ifcx-wasm/scripts/build-node-pkg.sh [out-dir]
#
# The wasm-bindgen CLI must match the `wasm-bindgen` crate version pinned in
# Cargo.toml exactly: a mismatch fails at bindgen time with a schema error,
# or worse, generates glue for a different ABI. This script refuses early and
# names the version to install.
set -euo pipefail

crate_dir="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$crate_dir/../.." && pwd)"
out="${1:-$crate_dir/pkg}"

pinned="$(sed -n 's/^wasm-bindgen = "=\(.*\)"$/\1/p' "$crate_dir/Cargo.toml")"
if [[ -z "$pinned" ]]; then
    echo "error: wasm-bindgen is not pinned with = in $crate_dir/Cargo.toml" >&2
    exit 1
fi
if ! command -v wasm-bindgen >/dev/null; then
    echo "error: wasm-bindgen CLI missing; run: cargo install wasm-bindgen-cli --version $pinned --locked" >&2
    exit 1
fi
installed="$(wasm-bindgen --version | awk '{print $2}')"
if [[ "$installed" != "$pinned" ]]; then
    echo "error: wasm-bindgen CLI $installed != crate $pinned; run: cargo install wasm-bindgen-cli --version $pinned --locked --force" >&2
    exit 1
fi

target_dir="${CARGO_TARGET_DIR:-$root/target}"
(cd "$root" && cargo build -p openbim-ifcx-wasm --target wasm32-unknown-unknown --release --locked)
rm -rf "$out"
wasm-bindgen --target nodejs --out-dir "$out" \
    "$target_dir/wasm32-unknown-unknown/release/openbim_ifcx_wasm.wasm"

# The generated glue is CommonJS. Without its own package.json, Node resolves
# the nearest enclosing one, which may say "type": "module" and break
# loading. The manifest also makes `out` a complete npm package.
crate_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$crate_dir/Cargo.toml" | head -1)"
npm_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$crate_dir/npm/package.json")"
if [[ "$crate_version" != "$npm_version" ]]; then
    echo "error: npm/package.json version $npm_version != Cargo.toml $crate_version" >&2
    exit 1
fi
cp "$crate_dir/npm/package.json" "$crate_dir/README.md" "$crate_dir/LICENSE" "$out/"

IFCX_WASM_PKG="$out" node --test "$crate_dir/tests/js/smoke.mjs"
