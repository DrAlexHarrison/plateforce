#!/usr/bin/env bash
#
# Every method id in a real result can be looked up, through the shipped CLI.
#
#   ./scripts/check-result-ids-resolve.sh
#
# The thing a stranger does with a fingerprint is take an id out of a methods section and
# look it up. This does exactly that, end to end: it analyses the one trace that is public,
# reads every id the result names, and asks `registry show` for each one. A build that emits
# an id nothing can resolve fails here, in the same order and by the same route the stranger
# would meet it.
#
# It runs the whole path rather than reading a table, so it also covers the ids a table does
# not reach: the rules computed from the landmarks, and the contributing ids each metric
# lists. Those are named in a result and a check that read only the binding table would clear
# a result that named one of them wrongly.
#
# Subject 01 is Michelle, and hers is the only trace that may be committed or published.

set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

trial="crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"
result="$(mktemp)"
ids="$(mktemp)"
trap 'rm -f "$result" "$ids"' EXIT

cargo build --quiet -p plateforce-cli
cli="target/debug/plateforce"

# The two compound names are chosen on purpose. They are the ones a caller may select and
# that no registry row carries, so they are where an unresolvable id would come from.
"$cli" analyse "$trial" \
  --column 0 \
  --sentinel none \
  --sample-rate-hz 1200 \
  --weighing bwepoch.fixed_window \
  --set weighing.duration=1.0 \
  --onset onset.threshold.last_within_band \
  --takeoff takeoff.threshold.longest_run \
  --derive jump_height.takeoff_frame=jumpheight.takeoff.impulse_momentum \
  --provenance \
  --format json >"$result"

python3 - "$result" "$ids" <<'PYTHON'
import json
import sys

result_path, ids_path = sys.argv[1], sys.argv[2]
with open(result_path) as handle:
    document = json.load(handle)

if "refusal" in document:
    sys.exit(f"the analysis refused, so there is no result to check: {document['refusal']}")

result = document.get("ok", document)
named = set()
for bound in result.get("bound_methods", []):
    named.add(bound["method_id"])
for metric in result.get("metrics", []):
    named.update(metric.get("contributing_method_ids") or [])
    if metric.get("computed_by"):
        named.add(metric["computed_by"])

with open(ids_path, "w") as handle:
    handle.write("\n".join(sorted(named)))
print(f"the result names {len(named)} distinct method ids")
PYTHON

named=0
unresolved=0
# The final line carries no newline, so a bare `read` returns false on it and the loop would
# skip the last id. That reads as a clean pass on the one id most likely to have been added
# last.
while read -r id || [ -n "$id" ]; do
  [ -n "$id" ] || continue
  named=$((named + 1))
  if ! "$cli" registry show "$id" >/dev/null 2>&1; then
    unresolved=$((unresolved + 1))
    echo "cannot be looked up: $id"
  fi
done <"$ids"

# A result naming nothing would pass every check below it, and reads exactly like a result
# whose every id resolved.
if [ "$named" -eq 0 ]; then
  echo "the result named no method ids at all, so this checked nothing" >&2
  exit 1
fi

if [ "$unresolved" -ne 0 ]; then
  echo "$unresolved of $named ids in this result resolve nowhere in the registry" >&2
  exit 1
fi

echo "all $named ids in this result resolve through registry show"
