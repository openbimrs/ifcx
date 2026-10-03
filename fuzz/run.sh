#!/usr/bin/env bash
# Runs one fuzz target for a bounded time with its seed inputs.
#
# Usage: fuzz/run.sh <target> [seconds] [extra libFuzzer flags...]
#        fuzz/run.sh --list
#
# New inputs go to fuzz/corpus/<target> (ignored by Git). Seeds are read
# from fuzz/seeds/<target> and, for targets that take IFCX files, from the
# hand-written fixtures in crates/*/tests/fixtures. A crash or timeout
# leaves its input in fuzz/artifacts/<target>/ and exits non-zero.
#
# Needs cargo-fuzz and the nightly toolchain pinned in
# fuzz/rust-toolchain.toml (installed on first use by rustup).
set -euo pipefail

cd "$(dirname "$0")/.."

targets=(read compose validate pcd pcd_lzf points_base64 geometry scene_glb)
if [[ "${1:-}" == "--list" ]]; then
    printf '%s\n' "${targets[@]}"
    exit 0
fi
target="${1:?usage: fuzz/run.sh <target> [seconds] [libFuzzer flags...]}"
seconds="${2:-60}"
shift $(( $# < 2 ? $# : 2 ))

# Read by rustup; must equal fuzz/rust-toolchain.toml.
toolchain="$(sed -n 's/^channel = "\(.*\)"/\1/p' fuzz/rust-toolchain.toml)"

seeds=()
case "$target" in
    read | compose | validate | scene_glb)
        seeds+=(crates/openbim-ifcx/tests/fixtures crates/openbim-ifcx-geometry/tests/fixtures) ;;
    pcd | pcd_lzf | points_base64 | geometry) ;;
    *) echo "error: unknown target '$target' (fuzz/run.sh --list)" >&2; exit 2 ;;
esac
[[ -d "fuzz/seeds/$target" ]] && seeds+=("fuzz/seeds/$target")
mkdir -p "fuzz/corpus/$target"

# Debug assertions turn integer overflow into a panic. -rss_limit_mb and -malloc_limit_mb turn unbounded allocation into a
# finding; -timeout turns a hang or super-linear blow-up into one.
exec cargo "+$toolchain" fuzz run --fuzz-dir fuzz -O --debug-assertions "$target" \
    "fuzz/corpus/$target" "${seeds[@]}" -- \
    -max_total_time="$seconds" -rss_limit_mb=2048 -malloc_limit_mb=1024 \
    -timeout=10 -max_len=65536 -dict=fuzz/ifcx.dict -print_final_stats=1 "$@"
