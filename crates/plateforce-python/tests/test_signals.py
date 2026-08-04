"""What the software noticed about a number, reaching a notebook.

A signal is not a refusal and not a warning. The number stands, and the signal carries the
action a reader would take. Python had none of them at all, on the surface most of this
field's research programmers use.

Two of the cases here are trials rather than assertions about one trial: a recording that
raises a signal, and the same recording as taken, which raises none. A test that only ever
saw a signal would pass against a surface that reported one on everything.
"""

import json
import os

import numpy as np
import pytest

import plateforce as pf

FIXTURE = os.path.join(
    os.path.dirname(__file__), "..", "..", "plateforce-conformance", "fixtures",
    "subject01_trial1.force.txt",
)

TAKEOFF_FRAME = "jump_height_from_takeoff_meters"
FLIGHT_TIME = "jump_height_from_flight_time_meters"

# Where the flight-time route reaches on this trial once the landing is placed late enough
# for the two routes to differ past the published difference between them. Measured, not
# picked: at the true touchdown the two are 6.7 percent apart and no signal is raised.
A_TOUCHDOWN_PLACED_LATE = 5900


@pytest.fixture
def shipped():
    return pf.Registry.load()


@pytest.fixture
def rules(shipped):
    return dict(
        weighing_epoch=shipped.method("bwepoch.fixed_window").bind(duration=1.0),
        onset=shipped.method("onset.threshold.noise_relative").bind(k=5.0),
        takeoff=shipped.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0),
    )


@pytest.fixture
def recorded():
    if not os.path.exists(FIXTURE):
        pytest.skip("this checkout carries no trial to read")
    return pf.read_force_file(FIXTURE, sample_rate_hz=1200.0, delimiter="\t", force_column=0)


@pytest.fixture
def truncated(recorded):
    """The recording stopped during the flight, so there is no landing and no second route."""
    return pf.Trial(np.asarray(recorded.force_newtons[:5200]), sample_rate_hz=1200.0)


def test_a_trial_nothing_was_noticed_about_reports_no_signal(recorded, rules):
    """The other side of every case below. Without this the suite would pass against a
    surface that raised a signal on every trial it was handed."""
    result = pf.analyse_countermovement_jump(recorded, **rules)
    assert result.signals == []
    assert result.signals_qualifying(TAKEOFF_FRAME) == []


def test_two_routes_disagreeing_reach_a_reader_with_the_figure_they_disagree_by(
    recorded, rules
):
    result = pf.analyse_countermovement_jump(
        recorded, touchdown_index=A_TOUCHDOWN_PLACED_LATE, **rules
    )
    assert len(result.signals) == 1
    signal = result.signals[0]
    assert signal.status == "disagrees"
    assert signal.value == pytest.approx(38.5677807, rel=1e-6)
    assert signal.value > signal.threshold, "a signal raised below its own threshold"
    assert signal.unit == "percent"
    assert signal.threshold == 20.0


def test_a_comparison_that_could_not_run_reports_no_value_rather_than_a_sentence(
    truncated, rules
):
    """A comparison that could not run, which the three rendering surfaces put into a
    sentence for a person to read. This surface reports what the signal holds and writes no
    sentence, so a caller branching on the status reads the record rather than prose."""
    result = pf.analyse_countermovement_jump(truncated, **rules)
    assert len(result.signals) == 1
    signal = result.signals[0]
    assert signal.status == "incomparable"
    assert signal.value is None
    assert signal.threshold == 20.0, "the threshold stands whether or not the check ran"
    assert "None" in repr(signal), "the repr states the value is absent"
    for invented in ("not comparable", "no second route"):
        assert invented not in repr(signal), f"the repr asserts {invented!r} of its own"


def test_the_two_statuses_are_told_apart(recorded, truncated, rules):
    """Silence and a check that could not run read the same to a caller who cannot tell them
    apart, which is the whole reason the second status exists."""
    disagrees = pf.analyse_countermovement_jump(
        recorded, touchdown_index=A_TOUCHDOWN_PLACED_LATE, **rules
    ).signals[0]
    incomparable = pf.analyse_countermovement_jump(truncated, **rules).signals[0]
    assert disagrees.status != incomparable.status
    assert {disagrees.status, incomparable.status} == {"disagrees", "incomparable"}


def test_a_signal_names_an_action_and_the_construct_behind_it(recorded, rules):
    result = pf.analyse_countermovement_jump(
        recorded, touchdown_index=A_TOUCHDOWN_PLACED_LATE, **rules
    )
    signal = result.signals[0]
    assert signal.remedy, "a signal without an action is a diagnosis a reader cannot act on"
    assert signal.remedy_construct == "takeoff"
    # The affordance a notebook has in place of the browser's control: the construct is a
    # field, so the published alternatives are a comprehension away rather than a sentence
    # to parse.
    alternatives = [
        entry.id
        for entry in pf.Registry.load().methods()
        if entry.construct == signal.remedy_construct
    ]
    assert len(alternatives) > 1, "a remedy naming a construct with one rule is not a choice"


def test_a_signal_names_the_quantities_it_qualifies(recorded, rules):
    result = pf.analyse_countermovement_jump(
        recorded, touchdown_index=A_TOUCHDOWN_PLACED_LATE, **rules
    )
    signal = result.signals[0]
    assert set(signal.qualifies) == {TAKEOFF_FRAME, FLIGHT_TIME}
    assert result.signals_qualifying(TAKEOFF_FRAME) != []
    assert result.signals_qualifying(FLIGHT_TIME) != []
    assert result.signals_qualifying("time_to_takeoff_seconds") == []


def test_the_shaped_result_and_the_engine_document_report_one_set_of_signals(
    recorded, rules
):
    """The classes above are built from the record the engine raised rather than raised
    again here, so the two cannot report different things about one analysis."""
    result = pf.analyse_countermovement_jump(
        recorded, touchdown_index=A_TOUCHDOWN_PLACED_LATE, **rules
    )
    document = json.loads(
        pf._analyse_json(recorded, touchdown_index=A_TOUCHDOWN_PLACED_LATE, **rules)
    )["ok"]
    assert len(document["signals"]) == len(result.signals)
    for wire, shaped in zip(document["signals"], result.signals):
        assert wire["status"] == shaped.status
        assert wire["value"] == shaped.value
        assert wire["threshold"] == shaped.threshold
        assert wire["remedy"] == shaped.remedy
        assert wire["remedy_construct"] == shaped.remedy_construct
        assert wire["label"] == shaped.label
        assert wire["unit"] == shaped.unit
        assert list(wire["qualifies"]) == shaped.qualifies


def test_the_status_spelling_is_the_one_the_wire_carries(truncated, rules):
    """A caller branching on this and one reading the JSON are reading one decision."""
    result = pf.analyse_countermovement_jump(truncated, **rules)
    document = json.loads(pf._analyse_json(truncated, **rules))["ok"]
    assert document["signals"][0]["status"] == result.signals[0].status
