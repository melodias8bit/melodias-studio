#!/usr/bin/env bash
# Validates the repository agent skills: frontmatter, repository paths and the
# make targets and test names they cite. Run through `make skills-check`.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
skills_dir="$root/.agents/skills"
status=0

fail() {
  printf 'skills: %s\n' "$*" >&2
  status=1
}

if [ ! -d "$skills_dir" ]; then
  fail "missing $skills_dir"
  exit 1
fi

for skill in "$skills_dir"/*/SKILL.md; do
  [ -f "$skill" ] || continue
  name=$(basename "$(dirname "$skill")")

  if [ "$(sed -n '1p' "$skill")" != "---" ]; then
    fail "$name: missing frontmatter"
    continue
  fi

  meta=$(awk 'NR == 1 && $0 == "---" { inside = 1; next }
              inside && $0 == "---" { exit }
              inside { print }' "$skill")

  declared=$(printf '%s\n' "$meta" | sed -n 's/^name:[[:space:]]*//p' | head -n 1)
  description=$(printf '%s\n' "$meta" | sed -n 's/^description:[[:space:]]*//p' | head -n 1)

  if [ -z "$declared" ]; then
    fail "$name: missing name"
  elif [ "$declared" != "$name" ]; then
    fail "$name: name '$declared' does not match the folder"
  fi

  if [ -z "$description" ]; then
    fail "$name: missing description"
  elif [ "${#description}" -gt 1024 ]; then
    fail "$name: description is longer than 1024 characters"
  fi

  # Repo-owned skills follow the writing rules; registry skills are vendored.
  case "$name" in
    melodias-*)
      if grep -q '—' "$skill"; then
        fail "$name: contains an em dash"
      fi
      ;;
  esac

  while IFS= read -r ref; do
    case "$ref" in
      src/* | docs/* | dashboard/* | deploy/*) ;;
      config.example.toml | Makefile | Cargo.toml | skills-lock.json) ;;
      *) continue ;;
    esac
    case "$ref" in
      *'<'* | *'>'* | *'*'* | *'$'* | *'{'* | *'}'*) continue ;;
    esac
    [ -e "$root/$ref" ] && continue
    # The Remotion entry point lives inside the video project.
    [ "$ref" = "src/index.ts" ] && [ -e "$root/video/src/index.ts" ] && continue
    fail "$name: referenced path does not exist: $ref"
  done < <(grep -oE '`[A-Za-z0-9_./-]+`' "$skill" | tr -d '`' | sort -u)

  while IFS= read -r target; do
    grep -qE "^${target}:" "$root/Makefile" ||
      fail "$name: unknown make target: $target"
  done < <(grep -oE '`make [a-z0-9-]+' "$skill" | sed 's/^`make //' | sort -u)

  while IFS= read -r test_name; do
    case "$test_name" in
      test_name | name | module) continue ;;
    esac
    grep -rqE "fn ${test_name}\(" "$root/src" && continue
    # `cargo test <module>` filters by path, so a module name is valid too.
    [ -e "$root/src/$test_name" ] || [ -e "$root/src/$test_name.rs" ] && continue
    fail "$name: unknown test or module: $test_name"
  done < <(grep -oE 'cargo test [a-z0-9_]+' "$skill" | sed 's/^cargo test //' | sort -u)
done

if [ "$status" -ne 0 ]; then
  exit 1
fi

printf 'skills: ok\n'
