#!/usr/bin/env bash
# The terminal's answer to one committed request, analysed or swept.
#
# Every value comes from the request file rather than from a line written here, so this arm
# and the others cannot drift into asking different questions. Two arms differing on the
# request compare two answers to two questions and pass.
set -o errexit -o nounset -o pipefail

request="${PLATEFORCE_PARITY_REQUEST:?the harness names the request file}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mapfile -t argv < <(python3 - "$request" <<'PY'
import json
import sys

asked = json.load(open(sys.argv[1], encoding="utf-8"))

# A request carrying a `sweep` block asks how far the number moves, and one without it asks
# what the analysis reports. The same test in the same words is what Python's arm and the
# browser's make, so the three cannot drift into answering different kinds of question.
sweep = asked.get("sweep")

argv = ["--format", "json"]
if sweep is None:
    argv += ["analyse", asked["trial"]]
else:
    # The terminal takes `--slot` and this arm passes none, so it reads the steps off the
    # rules the request bound, varying every construct this build runs more than one rule
    # for. Told the slots outright it would be asked what the others were asked; left to read
    # them, its `axes_varied` is a second account of the same sweep, and the two meet in the
    # committed record.
    argv += ["spread", asked["trial"], "--quantity", sweep["quantity_key"]]
argv += [
    "--column", str(asked["force_column"]),
    "--sample-rate-hz", str(asked["sample_rate_hz"]),
    "--sentinel", asked["sentinel_convention"],
    "--delimiter", asked["delimiter"],
]
for slot in ("weighing", "onset", "takeoff"):
    argv += [f"--{slot}", asked[slot]["method_id"]]
    for name, value in sorted(asked[slot]["parameters"].items()):
        argv += ["--set", f"{slot}.{name}={value}"]
print("\n".join(argv))
PY
)

cd "$root"
exec cargo run -q -p plateforce-cli -- "${argv[@]}"
