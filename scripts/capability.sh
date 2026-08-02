#!/usr/bin/env bash
# Asks every listed surface what it can do and compares the answers against one committed
# file. Byte equality against a baseline rather than between surfaces: several surfaces wrong
# the same way pass a pairwise check, and a committed file makes every change a diff a
# reviewer sees.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
surfaces="$root/scripts/capability-surfaces.txt"
manifest="$root/CAPABILITY.json"
mode="${1:---check}"

rows=()
while IFS=$'\t' read -r name command; do
  [[ -z "${name// }" || "${name:0:1}" == "#" ]] && continue
  rows+=("$name"$'\t'"$command")
done < "$surfaces"

if [[ ${#rows[@]} -eq 0 ]]; then
  echo "no surface is listed in ${surfaces#"$root"/}" >&2
  exit 1
fi

ask() {
  local command="$1"
  ( cd "$root" && eval "$command" )
}

case "$mode" in
  --write)
    IFS=$'\t' read -r name command <<< "${rows[0]}"
    ask "$command" > "$manifest.partial"
    mv -f "$manifest.partial" "$manifest"
    echo "CAPABILITY.json written from $name"
    ;;
  --check)
    if [[ ! -f "$manifest" ]]; then
      echo "CAPABILITY.json is absent; write it with $0 --write" >&2
      exit 1
    fi
    expected="$(cat "$manifest")"
    matched=0
    for row in "${rows[@]}"; do
      IFS=$'\t' read -r name command <<< "$row"
      if ! actual="$(ask "$command")"; then
        echo "$name could not report" >&2
        exit 1
      fi
      if [[ "$actual" != "$expected" ]]; then
        echo "$name does not match CAPABILITY.json:" >&2
        diff <(printf '%s\n' "$expected" | python3 -m json.tool) \
             <(printf '%s\n' "$actual" | python3 -m json.tool) >&2 || true
        exit 1
      fi
      matched=$((matched + 1))
    done
    echo "$matched of ${#rows[@]} surfaces reported and matched CAPABILITY.json"
    ;;
  *)
    echo "usage: $0 [--check|--write]" >&2
    exit 1
    ;;
esac
