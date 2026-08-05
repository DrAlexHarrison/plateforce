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


def block_of(members):
    """Members through the block's own constructor, so a name the block does not hold raises
    here rather than being dropped on the way past."""
    members = dict(members or {})
    frequency = members.pop("plate_natural_frequency_hz", None)
    return pf.Acquisition(
        plate_natural_frequency_hz=None if frequency is None else float(frequency),
        **members,
    )


def capture_of(capture):
    """What the plate was, and which saved plate the answers were typed into.

    The plate travels as its name and its members rather than as a path, because the request
    is answered on four machines and none of them saved it. `pf.Plate` takes the revision from
    the members, so the attribution this arm produces is the one the terminal produces off its
    own store. Naming a plate this machine has saved is `pf.Plate.saved`, which the terminal's
    `plateforce plate save` writes and this arm does not need.
    """
    if capture is None:
        return None, None
    stated = block_of(capture.get("acquisition"))
    named = capture.get("plate")
    if named is None:
        return stated, None
    return stated, pf.Plate(named["name"], block_of(named.get("members")))

convention = asked["sentinel_convention"]
if convention not in SENTINELS:
    raise SystemExit(f"plateforce: {convention} is not a convention this arm can state")
declared = SENTINELS[convention]

stated, named_plate = capture_of(asked.get("capture"))

trial = pf.read_force_file(
    asked["trial"],
    sample_rate_hz=asked["sample_rate_hz"],
    delimiter=asked["delimiter"],
    force_column=asked["force_column"],
    sentinel=None if declared is None else declared(),
    acquisition=stated,
    plate=named_plate,
)

registry = pf.Registry.load()
bound = {
    slot: registry.method(asked[slot]["method_id"]).bind(**asked[slot]["parameters"])
    for slot in ("weighing", "onset", "takeoff")
}

# Rules computed from the landmarks, keyed by the construct each fills. The values travel into
# the binding rather than beside it, because binding is where this surface checks that a rule
# whose entry publishes no default for a required name has been given one.
asked_derived = asked.get("derived", {})
derived = {}
if asked_derived:
    derived["derived"] = {
        construct: registry.method(choice["method_id"]).bind(
            **choice.get("parameters", {}), **choice.get("options", {})
        )
        for construct, choice in asked_derived.items()
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
            **derived,
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
