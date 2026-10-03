#!/usr/bin/env bash
# Records the branch heads of an upstream repository and reports which
# branches moved, appeared, or disappeared since a previous record.
#
#   scripts/upstream-drift/branches.sh <repository URL> <previous heads.tsv | ""> <new heads.tsv>
#
# A heads file has one "<branch>\t<sha>" line per branch, sorted by branch,
# and a `HEAD` line for the default branch.
# Prints a Markdown section to stdout. The `Upstream drift` workflow keeps
# the record as the `upstream-drift-state` workflow artifact.
set -euo pipefail

[ $# -eq 3 ] || { echo "usage: $0 <repository URL> <previous heads.tsv | \"\"> <new heads.tsv>" >&2; exit 2; }
url=$1 previous=$2 current=$3

# HEAD is the default branch's head; branches are listed under their names.
git ls-remote "$url" HEAD 'refs/heads/*' \
  | awk '{ sub("^refs/heads/", "", $2); print $2 "\t" $1 }' | LC_ALL=C sort >"$current"
[ -s "$current" ] || { echo "no branches at $url" >&2; exit 1; }

repo=${url%.git}
repo=${repo#https://github.com/}
commit() { printf '[`%s`](https://github.com/%s/commit/%s)' "${1:0:7}" "$repo" "$1"; }

if [ -z "$previous" ] || [ ! -s "$previous" ]; then
  echo "No earlier record of upstream branch heads; recorded $(wc -l <"$current") branches."
  exit 0
fi

lines=$(LC_ALL=C join -t $'\t' -a 1 -a 2 -e - -o 0,1.2,2.2 "$previous" "$current" \
  | while IFS=$'\t' read -r branch old new; do
      if [ "$old" = - ]; then
        echo "| \`$branch\` | new | $(commit "$new") |"
      elif [ "$new" = - ]; then
        echo "| \`$branch\` | deleted (was $(commit "$old")) | |"
      elif [ "$old" != "$new" ]; then
        echo "| \`$branch\` | moved from $(commit "$old") | $(commit "$new") ([compare](https://github.com/$repo/compare/$old...$new)) |"
      fi
    done)
if [ -z "$lines" ]; then
  echo "No upstream branch moved since the last recorded run ($(wc -l <"$current") branches)."
else
  echo "Upstream branches changed since the last recorded run:"
  echo
  echo "| Branch | Change | Head |"
  echo "| --- | --- | --- |"
  echo "$lines"
fi
