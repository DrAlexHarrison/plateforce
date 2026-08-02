#!/usr/bin/env bash
# Computes one result on every listed surface and holds each to one committed document.
#
# The manifest gate asks what a surface says it can do. This asks what it computes, which is
# the claim a reader acts on: the number pasted into a paper comes from one of these surfaces
# and a reader has no way to tell which. `scripts/result_parity.py` states what is asserted
# and what a green here does not prove.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
surfaces="$root/scripts/result-parity-surfaces.txt"
baseline="$root/tests/golden/result-parity.json"
export PLATEFORCE_PARITY_REQUEST="$root/tests/golden/result-parity-request.json"
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

collected=()
for row in "${rows[@]}"; do
  IFS=$'\t' read -r name command <<< "$row"
  if ! ( cd "$root" && eval "$command" ) > "$answers/$name.json" 2> "$answers/$name.err"; then
    echo "$name could not compute the result:" >&2
    tail -5 "$answers/$name.err" >&2
    exit 1
  fi
  collected+=("$name=$answers/$name.json")
done

python3 "$root/scripts/result_parity.py" "${mode#--}" "$baseline" "${collected[@]}"
