#!/usr/bin/env bash
# Computes every listed request on every listed surface and holds each to one committed record.
#
# The manifest gate asks what a surface says it can do. This asks what it computes, which is
# the claim a reader acts on: the number pasted into a paper comes from one of these surfaces
# and a reader has no way to tell which. `scripts/result_parity.py` states what is asserted
# and what a green here does not prove.
#
# Two manifests, because the gate has two populations and neither is a line in this file:
# `result-parity-surfaces.txt` names who is asked, `result-parity-requests.txt` names what
# they are asked. The Python reads both as well, so the harness cannot ask one set of either
# and report about another. Which surfaces answer which request is the request manifest's
# fourth column, read here and read again there for the same reason.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
surfaces="$root/scripts/result-parity-surfaces.txt"
requests="$root/scripts/result-parity-requests.txt"
mode="${1:---check}"
# Which surface a regeneration writes from, named rather than taken alphabetically. Only read
# in --write, and only needed where the surfaces disagree, which is where taking the first one
# would write a defect into the record meant to catch it.
source_surface="${2:-}"

read_manifest() {
  local line
  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "${line// }" || "${line:0:1}" == "#" ]] && continue
    printf '%s\n' "$line"
  done < "$1"
}

mapfile -t surface_rows < <(read_manifest "$surfaces")
mapfile -t request_rows < <(read_manifest "$requests")

if [[ ${#surface_rows[@]} -eq 0 ]]; then
  echo "no surface is listed in ${surfaces#"$root"/}" >&2
  exit 1
fi
if [[ ${#request_rows[@]} -eq 0 ]]; then
  echo "no request is listed in ${requests#"$root"/}" >&2
  exit 1
fi

case "$mode" in
  --write|--check) ;;
  *) echo "usage: $0 [--check|--write [surface]]" >&2; exit 1 ;;
esac

answers="$(mktemp -d)"
trap 'rm -rf "$answers"' EXIT

for request_row in "${request_rows[@]}"; do
  IFS=$'\t' read -r request_name request_path _ request_surfaces <<< "$request_row"
  if [[ ! -f "$root/$request_path" ]]; then
    echo "the request manifest names $request_path and there is no such file" >&2
    exit 1
  fi
  export PLATEFORCE_PARITY_REQUEST="$root/$request_path"

  for surface_row in "${surface_rows[@]}"; do
    IFS=$'\t' read -r surface command <<< "$surface_row"
    # Asked of the surfaces the row names and no others. A surface left off is one that
    # cannot be asked this question at all, which `SURFACES_NOT_ASKED` states with the work
    # that would change it; running it here would collect an answer to a different question
    # and hand the Python a document to compare.
    if [[ ",$request_surfaces," != *",$surface,"* ]]; then
      continue
    fi
    answer="$answers/$request_name.$surface.json"
    complaint="$answers/$request_name.$surface.err"

    # The exit status is recorded and carried rather than acted on here. A terminal meeting a
    # recording that lacks what a rule looks for writes the partial result and exits 65,
    # `Fault::Recording` in crates/plateforce-cli/src/exit.rs, with nothing on the other
    # stream. Read as a failure to compute, that ended the whole run before any surface was
    # compared, and printed five lines of an empty file as the reason. A surface has answered
    # when it produced a document; whether it also reported a deficient recording is a fact
    # about the recording, and it belongs beside the comparison rather than in front of it.
    set +o errexit
    ( cd "$root" && eval "$command" ) > "$answer" 2> "$complaint"
    status=$?
    set -o errexit
    printf '%s\n' "$status" > "$answers/$request_name.$surface.status"

    if [[ ! -s "$answer" ]]; then
      echo "$surface produced no answer to $request_name and exited $status:" >&2
      tail -5 "$complaint" >&2
      exit 1
    fi
  done
done

python3 "$root/scripts/result_parity.py" "${mode#--}" "$answers" ${source_surface:+"$source_surface"}
