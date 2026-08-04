#!/usr/bin/env bash
# Asks every listed surface what it can do and records every answer in one committed file.
#
# The answers are held under their own names rather than compared to a single document. What
# a surface dispatches and what it can write are different sets, so one document for all of
# them would be satisfied only by a surface claiming a capability it does not have, in the one
# file built to make such a claim visible. `capability_manifest.py` states what is asserted.
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

case "$mode" in
  --write|--check) ;;
  *) echo "usage: $0 [--check|--write]" >&2; exit 1 ;;
esac

answers="$(mktemp -d)"
trap 'rm -rf "$answers"' EXIT

# A surface listed and unreachable is a failure, never a skip: a gate that passes with fewer
# surfaces than it names is not a gate.
collected=()
for row in "${rows[@]}"; do
  IFS=$'\t' read -r name command <<< "$row"
  if ! ( cd "$root" && eval "$command" ) > "$answers/$name.json"; then
    echo "$name could not report" >&2
    exit 1
  fi
  collected+=("$name=$answers/$name.json")
done

python3 "$root/scripts/capability_manifest.py" "${mode#--}" "$manifest" "${collected[@]}"
