#!/usr/bin/env bash
# Opt-in parity check: composes every example of a local
# buildingSMART/IFC5-development checkout, and this crate's fixtures, with
# both upstream's TypeScript and openbim-ifcx, and compares the trees.
#
#   IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/upstream-parity.sh
#
# Needs Node.js >= 22 and npm. The first run installs the pinned esbuild from
# scripts/parity/package-lock.json (the only network access). Upstream's
# TypeScript is bundled from the checkout at run time into target/parity/;
# no upstream file is copied into this repository. Not part of gate.sh.
# Prints a Markdown table, one row per case; exit code 1 if any case differs.
set -euo pipefail

cd "$(dirname "$0")/.."

: "${IFCX_UPSTREAM_DIR:?set IFCX_UPSTREAM_DIR to a buildingSMART/IFC5-development checkout}"
upstream=$(cd "$IFCX_UPSTREAM_DIR" && pwd)
workflows="$upstream/src/ifcx-core/workflows.ts"
[ -f "$workflows" ] || { echo "no $workflows" >&2; exit 2; }

if [ ! -x scripts/parity/node_modules/.bin/esbuild ]; then
  npm ci --prefix scripts/parity --no-audit --no-fund >&2
fi

out=target/parity
mkdir -p "$out"
printf 'export { LoadIfcxFile } from %s;\n' "\"$workflows\"" > "$out/upstream-entry.ts"
scripts/parity/node_modules/.bin/esbuild "$out/upstream-entry.ts" --bundle --platform=node \
  --format=cjs --log-level=warning --outfile="$out/upstream.cjs"

cargo build --release --quiet -p openbim-ifcx --example compose-json

revision=$(git -C "$upstream" rev-parse --short HEAD 2>/dev/null || echo unknown)
echo "Upstream: buildingSMART/IFC5-development at $revision" >&2
exec node scripts/parity/parity.mjs "$PWD/$out/upstream.cjs" "$PWD/target/release/examples/compose-json" \
  "$upstream" "$PWD/crates/openbim-ifcx/tests/fixtures"
