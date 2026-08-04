#!/usr/bin/env bash
# The terminal's answer to one committed request, analysed or swept.
#
# Every value comes from the request file rather than from a line written here, so this arm
# and the others cannot drift into asking different questions. Two arms differing on the
# request compare two answers to two questions and pass.
#
# A request states a plate by its members, never by a name this machine happens to hold: the
# plate is saved into a folder this run makes and removes, and the revision a record carries
# is a digest over those members, so the committed record is the same on any machine. Every
# run of this arm passes `--plates`, including the requests that state no plate, because a run
# that fell through to the folder this machine keeps its settings in would answer for whatever
# the person at this keyboard last saved under the name.
set -o errexit -o nounset -o pipefail

request="${PLATEFORCE_PARITY_REQUEST:?the harness names the request file}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The plate the request states, in the words `plate save` takes it, and nothing at all where
# the request states none.
mapfile -t plate_argv < <(python3 - "$request" <<'PY'
import json
import sys

asked = json.load(open(sys.argv[1], encoding="utf-8"))
plate = (asked.get("capture") or {}).get("plate")
if plate is not None:
    argv = ["--format", "json", "plate", "save", plate["name"]]
    for member, value in sorted(plate["members"].items()):
        argv += ["--acquisition", f"{member}={value}"]
    print("\n".join(argv))
PY
)

mapfile -t argv < <(python3 - "$request" <<'PY'
import json
import sys

asked = json.load(open(sys.argv[1], encoding="utf-8"))

# A request carrying a `sweep` block asks how far the number moves, and one without it asks
# what the analysis reports. The same test in the same words is what Python's arm and the
# browser's make, so the three cannot drift into answering different kinds of question.
sweep = asked.get("sweep")
capture = asked.get("capture")

argv = ["--format", "json"]
if sweep is None:
    argv += ["analyse", asked["trial"]]
elif capture is not None:
    # A swept document declares no acquisition block and no attribution, so a sweep told
    # about a plate would be told something nothing it writes can carry. Said here rather
    # than dropped on the way past, which is how a request comes to ask one thing and a
    # surface to answer another.
    raise SystemExit(
        "plateforce: this request states a plate and asks a sweep, and `spread` writes no "
        "acquisition block for one to fill"
    )
else:
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
if capture is not None:
    plate = capture.get("plate")
    if plate is not None:
        argv += ["--plate", plate["name"]]
    # Stated beside the plate rather than in place of it: the member written here is the
    # answer that runs and the record says what it displaced, which is the half of the
    # feature a request naming a plate alone never reaches.
    for member, value in sorted(capture.get("acquisition", {}).items()):
        argv += ["--acquisition", f"{member}={value}"]
print("\n".join(argv))
PY
)

cd "$root"

# Where this run's saved plates live, gone when it ends. Named rather than defaulted, so no
# run of this gate reads or writes the plates the person at this keyboard saved.
plates="$(mktemp -d)"
trap 'rm -rf "$plates"' EXIT

# The save writes to the other stream, because the caller reads this one as the answer.
if [[ ${#plate_argv[@]} -gt 0 ]]; then
  cargo run -q -p plateforce-cli -- --plates "$plates" "${plate_argv[@]}" >&2
fi

cargo run -q -p plateforce-cli -- --plates "$plates" "${argv[@]}"
