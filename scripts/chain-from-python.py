"""Python's tree behind every number of one analysis, as JSON on this stream.

The trace is read from the path in the first argument and the quantities to answer for are the
ones after it, so this arm answers the population the run was asked about rather than the one
it can reach. A quantity it cannot answer for is a key with no tree on this stream, and the
comparison fails on it rather than passing over a name that went missing.

The request goes through `analyse_countermovement_jump` and each tree through `value()`, which
are the calls a notebook makes, so what this prints is what a reader holds.

Run by the interpreter `scripts/install-python-wheel.sh` names, which carries the wheel built
from this tree. Whichever `plateforce` happens to be importable would answer for whatever was
last installed on the machine.
"""

import json
import sys

import plateforce as pf

trace = sys.argv[1]
quantities = sys.argv[2:]

registry = pf.Registry.load()
bound = {
    slot: registry.method(method_id).bind(**parameters)
    for slot, (method_id, parameters) in {
        "weighing": ("bwepoch.fixed_window", {"duration": 1.0}),
        "onset": ("onset.threshold.noise_relative", {"k": 5.0}),
        "takeoff": ("takeoff.threshold.absolute_force", {"threshold_n": 20.0}),
    }.items()
}

# `sentinel=None` is the "none" convention on this surface, which is the value the terminal
# spells `--sentinel none` and R spells `sentinel_convention = "none"`.
trial = pf.read_force_file(
    trace, sample_rate_hz=1200.0, delimiter="\t", force_column=0, sentinel=None
)
analysed = pf.analyse_countermovement_jump(
    trial,
    weighing_epoch=bound["weighing"],
    onset=bound["onset"],
    takeoff=bound["takeoff"],
)


def tree_of(provenance):
    """The two fields the comparison reads, walked to the bottom.

    Parameter names alone, because the comparison asks which choices a surface says produced a
    number rather than whether two surfaces agree on arithmetic, which the parity gate asks
    over the whole document.
    """
    return {
        "method_id": provenance.method_id,
        "parameters": sorted(provenance.parameters_of(provenance.method_id)),
        "depends_on": [tree_of(below) for below in provenance.depends_on],
    }


trees = {}
for quantity in quantities:
    measured = analysed.value(quantity)
    if measured is not None:
        trees[quantity] = tree_of(measured.provenance)

print(json.dumps(trees))
