#!/usr/bin/env bash
# Build the @openbim/ifcx npm package, then test every target it ships.
#
#   crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh [out-dir]
#
# One release build of the wasm module, bound three times by wasm-bindgen,
# as openbimrs/ifc's @openbim/ifc does:
#
#   <out>/            --target nodejs   CommonJS for Node (the `node` condition)
#   <out>/bundler/    --target bundler  ES module for webpack, Rollup (the default)
#   <out>/web/        --target web      ES module with `init()`, no bundler needed
#
# Each target also exports `fetchImports` from js/fetch-imports.js, the
# JavaScript-side import resolver (ADR 0002 keeps network access out of the
# Rust crates).
#
# Then the Node suite runs against <out>, and tools/check-package.mjs packs
# <out> as npm would publish it and checks each target from that tarball:
# Node `require` and `import`, a webpack bundle, and both browser builds in
# headless Chromium, which parse, validate, compose, fetch imports and
# export GLB from the repository's fixtures.
#
# The wasm-bindgen CLI must match the `wasm-bindgen` crate version pinned in
# Cargo.toml exactly: a mismatch fails at bindgen time with a schema error,
# or worse, generates glue for a different ABI. This script refuses early and
# names the version to install. webpack comes pinned from
# tools/package-lock.json.
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
crate_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$crate_dir/Cargo.toml" | head -1)"
npm_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$crate_dir/npm/package.json")"
if [[ "$crate_version" != "$npm_version" ]]; then
    echo "error: npm/package.json version $npm_version != Cargo.toml $crate_version" >&2
    exit 1
fi

(cd "$crate_dir/tools" && npm ci --no-audit --no-fund --loglevel=error)

target_dir="${CARGO_TARGET_DIR:-$root/target}"
(cd "$root" && cargo build -p openbim-ifcx-wasm --target wasm32-unknown-unknown --release --locked)
module="$target_dir/wasm32-unknown-unknown/release/openbim_ifcx_wasm.wasm"

rm -rf "$out"
wasm-bindgen --target nodejs --out-dir "$out" "$module"
wasm-bindgen --target bundler --out-dir "$out/bundler" "$module"
wasm-bindgen --target web --out-dir "$out/web" "$module"

# The nodejs glue is CommonJS. Without its own package.json, Node resolves
# the nearest enclosing one, which may say "type": "module" and break
# loading. The manifest also makes `out` a complete npm package. The two ES
# module targets declare themselves, so Node (and any tool that honours
# "type") reads them as modules although the package root says "commonjs".
cp "$crate_dir/npm/package.json" "$crate_dir/README.md" "$crate_dir/LICENSE" "$out/"
for dir in bundler web; do
    printf '{\n  "type": "module"\n}\n' >"$out/$dir/package.json"
done

# fetchImports: the ES module source as is for the two ES targets, and a
# CommonJS copy for Node with `export` dropped and the names assigned to
# `exports`, so Node's ESM loader still sees them as named exports.
js="$crate_dir/js/fetch-imports.js"
names="$(sed -n 's/^export \(async \)\{0,1\}function \([A-Za-z_]*\).*/\2/p' "$js")"
if [[ -z "$names" ]] || grep -n '^export ' "$js" | grep -vq 'function '; then
    echo "error: $js must export only top-level functions" >&2
    exit 1
fi
{
    echo '"use strict";'
    sed 's/^export //' "$js"
    for name in $names; do echo "exports.$name = $name;"; done
} >"$out/fetch-imports.js"
cp "$crate_dir/js/fetch-imports.d.ts" "$out/fetch-imports.d.ts"
for dir in bundler web; do
    cp "$js" "$crate_dir/js/fetch-imports.d.ts" "$out/$dir/"
done
for name in $names; do
    echo "exports.$name = require(\"./fetch-imports.js\").$name;" >>"$out/openbim_ifcx_wasm.js"
    for dir in bundler web; do
        echo "export { $name } from \"./fetch-imports.js\";" >>"$out/$dir/openbim_ifcx_wasm.js"
    done
done
for dts in "$out/openbim_ifcx_wasm.d.ts" "$out/bundler/openbim_ifcx_wasm.d.ts" "$out/web/openbim_ifcx_wasm.d.ts"; do
    echo 'export * from "./fetch-imports.js";' >>"$dts"
done

IFCX_WASM_PKG="$out" node --test "$crate_dir/tests/js/smoke.mjs" "$crate_dir/tests/js/guide.mjs"
node "$crate_dir/tools/check-package.mjs" "$out"
