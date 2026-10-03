#!/usr/bin/env bash
# Build the openbim-ifcx wheel with maturin, install it into a throwaway uv
# venv, and run the Python suites against it. Used by the gate and CI.
#
# Needs `uv` and `maturin` on PATH (CI pins both; see .github/workflows).
set -euo pipefail

crate_dir="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in uv maturin; do
    command -v "$tool" >/dev/null || { echo "error: $tool not on PATH" >&2; exit 1; }
done

uv venv --quiet --python "${PYTHON:-python3}" "$work/venv"

# A real release wheel, installed the way a user would get it.
maturin build --quiet --release --manifest-path "$crate_dir/Cargo.toml" --out "$work/dist"
uv pip install --quiet --python "$work/venv/bin/python" "$work"/dist/*.whl
ls "$work"/dist

# Run from outside the source tree so the tests import the installed wheel,
# not the python/ sources beside them. No pipe: the exit status is the verdict.
cd "$work"
"$work/venv/bin/python" -m unittest discover \
    -s "$crate_dir/tests/python" -t "$crate_dir/tests/python" -v
