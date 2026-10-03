#!/usr/bin/env bash
# Opt-in check: exports GLB files with the ifcx2glb example and validates
# them with the Khronos glTF validator.
#
#   ./scripts/gltf-validate.sh
#   IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/gltf-validate.sh
#
# Exports the hand-written fixtures of openbim-ifcx-geometry (with their
# relative imports resolved) and, with
# IFCX_UPSTREAM_DIR, every example of a local buildingSMART/IFC5-development
# checkout (a file that does not compose alone is composed on top of the
# other files of its example folder). GLB files go to target/gltf/.
#
# Needs Node.js >= 18 and npm. The first run installs the pinned
# gltf-validator from scripts/gltf/package-lock.json (the only network
# access); set GLTF_VALIDATOR_DIR to a directory whose node_modules already
# holds gltf-validator to skip that. Not part of gate.sh. Exit code 1 if any
# file has a validation error.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -z "${GLTF_VALIDATOR_DIR:-}" ] && [ ! -d scripts/gltf/node_modules/gltf-validator ]; then
  npm ci --prefix scripts/gltf --no-audit --no-fund >&2
fi

cargo build --release --quiet -p openbim-ifcx-geometry --example ifcx2glb
convert=target/release/examples/ifcx2glb
out=target/gltf
rm -rf "$out"
mkdir -p "$out/fixtures" "$out/upstream"

# Fixtures may import other fixtures by relative path (imports-panel-type).
for f in crates/openbim-ifcx-geometry/tests/fixtures/*.ifcx; do
  "$convert" --resolve-imports "$f" "$out/fixtures/$(basename "${f%.ifcx}").glb" >/dev/null
done

if [ -n "${IFCX_UPSTREAM_DIR:-}" ]; then
  examples=$(cd "$IFCX_UPSTREAM_DIR" && pwd)/examples
  while IFS= read -r -d '' f; do
    rel=${f#"$examples"/}
    glb="$out/upstream/$(printf '%s' "${rel%.ifcx}" | tr '/ ' '__').glb"
    if ! "$convert" "$f" "$glb" 2>/dev/null; then
      folder="$examples/${rel%%/*}"
      layers=()
      while IFS= read -r -d '' other; do
        [ "$other" != "$f" ] && layers+=("$other")
      done < <(find "$folder" -name '*.ifcx' -print0 | sort -z)
      "$convert" "${layers[@]}" "$f" "$glb" >/dev/null \
        || { echo "cannot export $rel, alone or on its folder" >&2; exit 1; }
    fi
  done < <(find "$examples" -name '*.ifcx' -print0 | sort -z)
fi

shopt -s nullglob
exec node scripts/gltf/validate.mjs "$out"/fixtures/*.glb "$out"/upstream/*.glb
