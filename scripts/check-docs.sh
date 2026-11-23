#!/usr/bin/env bash
# Validates the make targets cited in docs exist in the Makefile.
# Run through `make docs-check`.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
status=0

fail() {
  printf 'docs: %s\n' "$*" >&2
  status=1
}

targets=$(grep -m1 '^\.PHONY' "$root/Makefile" | sed 's/^\.PHONY: //')
ignore='^(the|a|an|two|sure|sense|target|targets|it|this|that|them)$'

shopt -s globstar nullglob
files=("$root"/docs/**/*.md "$root"/dashboard/README.md)
for file in "${files[@]}"; do
  while IFS=: read -r line match; do
    target=${match#make }
    if [[ $target =~ $ignore ]]; then
      continue
    fi
    if [[ " $targets " != *" $target "* ]]; then
      fail "${file#$root/}:$line: unknown make target '$target'"
    fi
  done < <(grep -nEo 'make [a-z][a-z-]*' "$file" || true)
done

if [ "$status" -eq 0 ]; then
  echo "docs: ok"
fi
exit "$status"
