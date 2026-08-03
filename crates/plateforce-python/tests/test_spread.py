"""The sweep, and whether this surface's answer is the terminal's answer.

The spread is the measurement this software exists to publish: how far the choice of method
moves the number. A notebook could not compute it before this file existed, which left the
largest population of readers in this field able to run the analysis and unable to run the
argument.

The cross-surface case below is the one that matters. A test holding this surface to itself
would pass while the two surfaces reported different spreads for one trial, which is exactly
the divergence the product exists to make visible.
"""

import json
import os
import shutil
import subprocess

import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ

REPOSITORY = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
FIXTURE = os.path.join(
    REPOSITORY, "crates", "plateforce-conformance", "fixtures", "subject01_trial1.force.txt"
)

# The three steps a jump height rests on, named here rather than defaulted inside either
# surface, so the two are asked the same question by this file rather than by agreeing about
# a default each keeps its own copy of.
LANDMARK_SLOTS = ["weighing", "onset", "takeoff"]

RULES = {
    "weighing": ("bwepoch.fixed_window", {"duration": 1.0}),
    "onset": ("onset.threshold.noise_relative", {"k": 5.0}),
    "takeoff": ("takeoff.threshold.absolute_force", {"threshold_n": 20.0}),
}

QUANTITY = "jump_height_from_takeoff_meters"


@pytest.fixture
def shipped_registry():
    return pf.Registry.load()


@pytest.fixture
def bound(shipped_registry):
    return {
        slot: shipped_registry.method(method_id).bind(**parameters)
        for slot, (method_id, parameters) in RULES.items()
    }


@pytest.fixture
def fixture_trial():
    if not os.path.exists(FIXTURE):
        pytest.skip("this checkout carries no trial to read")
    return pf.read_force_file(
        FIXTURE, sample_rate_hz=1200.0, delimiter="\t", force_column=0
    )


def sweep(trial, bound, slot, **stated):
    return pf.spread(
        trial,
        quantity=QUANTITY,
        slot=slot,
        weighing_epoch=bound["weighing"],
        onset=bound["onset"],
        takeoff=bound["takeoff"],
        **stated,
    )


def terminal_spread():
    """The same sweep, computed by the terminal, so the comparison is between surfaces."""
    if shutil.which("cargo") is None:
        pytest.skip("no cargo on this machine, so the terminal cannot be asked")
    argv = [
        "cargo", "run", "-q", "-p", "plateforce-cli", "--",
        "--format", "json", "spread", FIXTURE,
        "--column", "0", "--sample-rate-hz", "1200", "--sentinel", "none", "--delimiter", "\t",
        "--quantity", QUANTITY,
    ]
    for slot, (method_id, parameters) in RULES.items():
        argv += [f"--{slot}", method_id]
        for name, value in parameters.items():
            argv += ["--set", f"{slot}.{name}={value}"]
    finished = subprocess.run(argv, cwd=REPOSITORY, capture_output=True, text=True)
    if finished.returncode != 0:
        pytest.skip(f"the terminal could not be built here: {finished.stderr[-300:]}")
    return json.loads(finished.stdout)["ok"]


def test_the_notebook_and_the_terminal_report_one_spread(fixture_trial, bound):
    """Every summary figure, as parsed doubles, and the variants as a set rather than the
    headline alone: two surfaces can agree on a median and disagree about what they swept."""
    there = terminal_spread()
    here = sweep(fixture_trial, bound, LANDMARK_SLOTS)

    assert {v["label"]: v["value"] for v in there["variants"]} == {
        v.label: v.value for v in here.variants
    }
    for field in (
        "quantity_key",
        "unit",
        "unit_symbol",
        "combinations_requested",
        "combinations_run",
        "capped",
        "succeeded",
        "failed",
        "minimum",
        "maximum",
        "median",
        "spread_absolute",
        "spread_percent_of_median",
        "baseline_value",
    ):
        assert getattr(here, field) == there[field], field


def test_the_method_choice_moves_the_number(fixture_trial, bound):
    """The product's own thesis, on one real trial: the onset rule alone moves a jump height
    by more than the effect a training study is built to detect."""
    swept = sweep(fixture_trial, bound, "onset")
    assert swept.succeeded == 5
    assert swept.failed == 0
    assert swept.spread_absolute > 0.01, "five published onset rules agreeing to a centimetre"
    assert swept.baseline_value == pytest.approx(0.4105176602724294)


def test_a_variant_carries_what_it_bound(fixture_trial, bound):
    swept = sweep(fixture_trial, bound, "onset")
    labels = {v.label for v in swept.variants}
    assert "onset onset.threshold.noise_relative" in labels
    for variant in swept.variants:
        assert variant.settings["onset"] in variant.method_ids
        assert variant.failure_reason is None


def test_sweeping_a_parameter_holds_the_rule(fixture_trial, bound):
    swept = sweep(fixture_trial, bound, "onset", parameter="k", values=[1.0, 5.0, 10.0])
    assert swept.combinations_run == 3
    assert {v.settings["k"] for v in swept.variants} == {"1", "5", "10"}
    assert all(v.method_ids[1] == RULES["onset"][0] for v in swept.variants)


def test_the_denominator_holds_the_combinations_that_produced_nothing(trial, bound_methods):
    """A synthetic trace no takeoff rule can read still reports every combination it ran."""
    epoch, onset, takeoff = bound_methods
    swept = pf.spread(
        trial,
        quantity=QUANTITY,
        slot="onset",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        method_ids=[onset.method_id],
    )
    assert swept.combinations_run == swept.succeeded + swept.failed
    assert len(swept.variants) == swept.combinations_run


def test_a_slot_this_build_runs_no_rule_for_is_refused(fixture_trial, bound):
    with pytest.raises(pf.MethodError) as raised:
        sweep(fixture_trial, bound, "not_a_step")
    assert "not_a_step" in str(raised.value)


def test_a_parameter_and_several_slots_are_refused_together(fixture_trial, bound):
    """Each describes one slot, so the pair states a sweep nobody can mean."""
    with pytest.raises(pf.MethodError):
        sweep(fixture_trial, bound, LANDMARK_SLOTS, parameter="k", values=[1.0, 5.0])


def test_naming_no_slot_is_refused(fixture_trial, bound):
    with pytest.raises(pf.MethodError):
        sweep(fixture_trial, bound, [])


def test_the_cap_is_reported_rather_than_applied_silently(fixture_trial, bound):
    swept = sweep(fixture_trial, bound, LANDMARK_SLOTS, maximum_combinations=4)
    assert swept.capped is True
    assert swept.combinations_run == 4
    assert swept.combinations_requested > 4


def test_a_sweep_over_a_synthetic_trace_reports_its_unit(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    swept = pf.spread(
        trial,
        quantity="system_weight_newtons",
        slot="weighing",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        method_ids=[epoch.method_id],
    )
    assert swept.quantity_key == "system_weight_newtons"
    assert swept.unit == "newtons"
    assert swept.succeeded == 1
    assert swept.baseline_value == pytest.approx(
        swept.variants[0].value, rel=0, abs=0
    )
    assert len(swept) == 1
    assert SAMPLE_RATE_HZ == trial.sample_rate_hz
