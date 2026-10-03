#!/usr/bin/env bash
# Runs every opt-in upstream check against a local
# buildingSMART/IFC5-development checkout and compares their summary lines
# with the recorded baseline. The scheduled `Upstream drift` workflow
# (.github/workflows/upstream-drift.yml) runs this weekly; it also works
# locally:
#
#   IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/upstream-drift.sh
#   IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/upstream-drift.sh --update-baseline
#
# Steps, each logged to target/upstream-drift/logs/<check>.log:
#
#   imports       fetch the ifcx.dev imports into an offline mirror
#                 (scripts/upstream-drift/fetch-imports.py; the only network
#                 access besides the pinned npm installs below)
#   round-trip    upstream_round_trip
#   validation    upstream_validation   (IFCX_IMPORTS_DIR from the mirror)
#   composition   upstream_composition
#   layers        upstream_layers       (IFCX_IMPORTS_MIRROR, feature fs)
#   decode        upstream_decode       (openbim-ifcx-geometry)
#   scene         upstream_scene        (openbim-ifcx-geometry)
#   parity        scripts/upstream-parity.sh   (Node.js >= 22, npm)
#   gltf          scripts/gltf-validate.sh     (Node.js >= 18, npm)
#
# A check fails when its command exits non-zero. Each check's summary line
# (file counts, failures, unresolved imports; timings removed) is written to
# target/upstream-drift/summary.txt and compared with
# scripts/upstream-drift/baseline.txt; any line that changed is a
# difference. --update-baseline rewrites the baseline from this run.
#
# Writes results.tsv (check, pass/fail, exit code), summary.txt,
# baseline.diff, revision, and report.md (Markdown for the drift issue) to
# target/upstream-drift/ (IFCX_DRIFT_OUT overrides it). Exit code 0 if every
# check passed and nothing differs from the baseline, 1 otherwise, 2 on
# usage errors. Not part of gate.sh.
set -uo pipefail

cd "$(dirname "$0")/.."
root=$PWD

update_baseline=
case "${1:-}" in
  "") ;;
  --update-baseline) update_baseline=1 ;;
  *) echo "usage: IFCX_UPSTREAM_DIR=... $0 [--update-baseline]" >&2; exit 2 ;;
esac

: "${IFCX_UPSTREAM_DIR:?set IFCX_UPSTREAM_DIR to a buildingSMART/IFC5-development checkout}"
upstream=$(cd "$IFCX_UPSTREAM_DIR" && pwd) || exit 2
out=${IFCX_DRIFT_OUT:-$root/target/upstream-drift}
mirror=${IFCX_IMPORTS_MIRROR:-$out/mirror}
baseline=$root/scripts/upstream-drift/baseline.txt
mkdir -p "$out/logs" "$mirror"
mirror=$(cd "$mirror" && pwd)
flat=$out/imports
rm -rf "$flat" "$out/logs"/* "$out"/{results.tsv,summary.txt,baseline.diff,report.md}
: >"$out/results.tsv"
: >"$out/summary.txt"

revision=$(git -C "$upstream" rev-parse HEAD 2>/dev/null || echo unknown)
echo "$revision" >"$out/revision"

group() { [ -n "${GITHUB_ACTIONS:-}" ] && echo "::group::$1" || echo "== $1" >&2; }
endgroup() { [ -n "${GITHUB_ACTIONS:-}" ] && echo "::endgroup::" || true; }

# run_check <name> <summary pattern> <command...>
# Runs the command, tees its output to the log, records pass/fail, and
# appends the log lines matching the extended regex to summary.txt (none if
# the pattern is empty).
run_check() {
  local name=$1 pattern=$2
  shift 2
  local log=$out/logs/$name.log
  group "$name"
  "$@" 2>&1 | tee "$log"
  local status=${PIPESTATUS[0]}
  endgroup
  if [ "$status" -eq 0 ]; then
    printf '%s\tpass\t0\n' "$name" >>"$out/results.tsv"
  else
    printf '%s\tfail\t%s\n' "$name" "$status" >>"$out/results.tsv"
    echo "::error::upstream check '$name' failed (exit $status)" >&2
  fi
  [ -n "$pattern" ] || return 0
  local lines
  lines=$(grep -E -- "$pattern" "$log" | sed -E 's/ in [0-9.]+(ns|µs|ms|s)//g')
  if [ -z "$lines" ]; then
    lines="no summary line (see logs/$name.log)"
  fi
  printf '%s\n' "$lines" | sed "s/^/$name: /" >>"$out/summary.txt"
}

gltf_summary() {
  # validate.mjs prints one "<n> errors, <n> warnings, <n> infos, <n> hints  <file>" line per file.
  awk '/^[0-9]+ errors, [0-9]+ warnings, [0-9]+ infos, [0-9]+ hints  / {
         n++; e += $1; w += $3; i += $5; h += $7 }
       END { if (n) printf "%d files: %d errors, %d warnings, %d infos, %d hints\n", n, e, w, i, h }' \
    "$out/logs/gltf.log" | sed 's/^/gltf: /' >>"$out/summary.txt"
}

export IFCX_UPSTREAM_DIR=$upstream

run_check imports '^([0-9]+ URIs, |import )' \
  python3 "$root/scripts/upstream-drift/fetch-imports.py" "$upstream" "$mirror" "$flat"
run_check round-trip '^[0-9]+ files, [0-9]+ failures$' \
  cargo test --release -p openbim-ifcx --test upstream_round_trip -- --nocapture
run_check validation '^[0-9]+ files: .* invalid$' \
  env IFCX_IMPORTS_DIR="$flat" \
  cargo test --release -p openbim-ifcx --test upstream_validation -- --nocapture
run_check composition '^[0-9]+ files: .* fail$' \
  cargo test --release -p openbim-ifcx --test upstream_composition -- --nocapture
run_check layers '(stacks built|^  unresolved )' \
  env IFCX_IMPORTS_MIRROR="$mirror" \
  cargo test --release -p openbim-ifcx --features fs --test upstream_layers -- --nocapture
run_check decode '^[0-9]+ files( in [^:]+)?: ' \
  cargo test --release -p openbim-ifcx-geometry --test upstream_decode -- --nocapture --test-threads 1
run_check scene '^[0-9]+ files in .* warnings$' \
  cargo test --release -p openbim-ifcx-geometry --test upstream_scene -- --nocapture
run_check parity '^[0-9]+ cases: ' "$root/scripts/upstream-parity.sh"
run_check gltf '' "$root/scripts/gltf-validate.sh"
gltf_summary
grep -q '^gltf: ' "$out/summary.txt" || echo "gltf: no summary line (see logs/gltf.log)" >>"$out/summary.txt"

if [ -n "$update_baseline" ]; then
  {
    echo "# Summary lines of scripts/upstream-drift.sh against"
    echo "# buildingSMART/IFC5-development $revision."
    echo "# Regenerate with --update-baseline after reviewing a drift report."
    cat "$out/summary.txt"
  } >"$baseline"
  echo "baseline updated: $baseline" >&2
fi

grep -v '^#' "$baseline" | diff -u --label baseline --label "upstream ${revision:0:7}" - "$out/summary.txt" \
  >"$out/baseline.diff"
differs=$?
failed=$(awk -F'\t' '$2 == "fail" { print $1 }' "$out/results.tsv" | paste -sd' ' -)
pinned=$(sed -n 's/^# buildingSMART\/IFC5-development \([0-9a-f]*\)\.$/\1/p' "$baseline")

{
  echo "Upstream: [buildingSMART/IFC5-development@${revision:0:7}](https://github.com/buildingSMART/IFC5-development/commit/$revision)"
  echo "(baseline recorded at \`${pinned:0:7}\`)."
  echo
  echo "| Check | Result |"
  echo "| --- | --- |"
  awk -F'\t' '{ printf "| `%s` | %s |\n", $1, ($2 == "pass" ? "pass" : "**FAIL** (exit " $3 ")") }' "$out/results.tsv"
  echo
  if [ "$differs" -ne 0 ]; then
    echo "Summary lines that differ from \`scripts/upstream-drift/baseline.txt\`:"
    echo
    echo '```diff'
    tail -n +3 "$out/baseline.diff" | head -n 80
    echo '```'
  else
    echo "Every summary line equals \`scripts/upstream-drift/baseline.txt\`."
  fi
  for name in $failed; do
    echo
    echo "<details><summary>Last lines of <code>$name</code></summary>"
    echo
    echo '```'
    grep -v '^\s*$' "$out/logs/$name.log" | sed -e "s|$upstream/|<upstream>/|g" -e "s|$root/||g" \
      | tail -n 20 | cut -c1-240
    echo '```'
    echo
    echo "</details>"
  done
} >"$out/report.md"

cat "$out/report.md"
if [ -n "$failed" ] || [ "$differs" -ne 0 ]; then
  exit 1
fi
