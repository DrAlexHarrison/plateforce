"""Python's answer to the one committed request.

Every value comes from the request file rather than from a line written here, so this arm
and the others cannot drift into asking different questions.

The request is turned into a call through the entry points a user's own call goes through:
the package reads the file and the package builds the request. An arm that parsed the export
itself, or assembled a request beside the one `analyse_countermovement_jump` sends, would be
comparing a document nobody sends and the gate would be measuring this file.
"""

import json
import os

import plateforce as pf

# How this surface spells a missing-sample convention. The request states the convention by
# the name the registry publishes and each arm reaches for its own surface's spelling of it,
# as the terminal's arm does with `--sentinel`.
SENTINELS = {
    "none": None,
    "zero": pf.Sentinel.zero,
    "negative_one": pf.Sentinel.negative_one,
}

with open(os.environ["PLATEFORCE_PARITY_REQUEST"], encoding="utf-8") as handle:
    asked = json.load(handle)

convention = asked["sentinel_convention"]
if convention not in SENTINELS:
    raise SystemExit(f"plateforce: {convention} is not a convention this arm can state")
declared = SENTINELS[convention]

trial = pf.read_force_file(
    asked["trial"],
    sample_rate_hz=asked["sample_rate_hz"],
    delimiter=asked["delimiter"],
    force_column=asked["force_column"],
    sentinel=None if declared is None else declared(),
)

registry = pf.Registry.load()
print(
    pf._analyse_json(
        trial,
        weighing_epoch=registry.method(asked["weighing"]["method_id"]).bind(
            **asked["weighing"]["parameters"]
        ),
        onset=registry.method(asked["onset"]["method_id"]).bind(**asked["onset"]["parameters"]),
        takeoff=registry.method(asked["takeoff"]["method_id"]).bind(
            **asked["takeoff"]["parameters"]
        ),
    )
)
