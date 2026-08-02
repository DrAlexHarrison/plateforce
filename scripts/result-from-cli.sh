#!/usr/bin/env bash
# The terminal's answer to the one committed request.
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
argv = [
    "--format", "json",
    "analyse", asked["trial"],
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
