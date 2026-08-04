"""Python's answer to one committed request, analysed or swept.

Every value comes from the request file rather than from a line written here, so this arm
and the others cannot drift into asking different questions.

The request is turned into a call through the entry points a user's own call goes through:
the package reads the file and the package builds the request. An arm that parsed the export
itself, or assembled a request beside the one `analyse_countermovement_jump` sends, would be
comparing a document nobody sends and the gate would be measuring this file. The sweep goes
the same way, through the run `plateforce.spread` makes rather than a second one built here.
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


def acquisition_of(capture):
    """What the plate was, as far as this surface can state it.

    A notebook holds no store of saved plates, so it types the answers a request states and
    lays the ones stated on the capture over the ones the plate holds, which is what the two
    surfaces reading a saved plate do with them. What it cannot say is which saved plate they
    came off: the record this surface produces carries the members and no attribution, and
    `plate_profile` in `scripts/result_parity.py` is where that gap is recorded with the work
    that closes it.

    Members reach the block through the block's own constructor, so a name the block does not
    hold raises here rather than being dropped on the way past.
    """
    if capture is None:
        return None
    members = dict((capture.get("plate") or {}).get("members") or {})
    members.update(capture.get("acquisition") or {})
    frequency = members.pop("plate_natural_frequency_hz", None)
    return pf.Acquisition(
        plate_natural_frequency_hz=None if frequency is None else float(frequency),
        **members,
    )

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
    acquisition=acquisition_of(asked.get("capture")),
)

registry = pf.Registry.load()
bound = {
    slot: registry.method(asked[slot]["method_id"]).bind(**asked[slot]["parameters"])
    for slot in ("weighing", "onset", "takeoff")
}

# A request carrying a `sweep` block asks how far the number moves, and one without it asks
# what the analysis reports. The terminal's arm and the browser's make the same test in the
# same words.
sweep = asked.get("sweep")
if sweep is None:
    print(
        pf._analyse_json(
            trial,
            weighing_epoch=bound["weighing"],
            onset=bound["onset"],
            takeoff=bound["takeoff"],
        )
    )
else:
    print(
        pf._spread_json(
            trial,
            quantity=sweep["quantity_key"],
            slot=sweep["slots"],
            weighing_epoch=bound["weighing"],
            onset=bound["onset"],
            takeoff=bound["takeoff"],
            maximum_combinations=sweep["maximum_combinations"],
        )
    )
