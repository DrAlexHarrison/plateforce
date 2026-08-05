"""The sweep, on a trace this file makes rather than one the checkout may or may not carry.

The spread is the measurement this software exists to publish: how far the choice of method
moves the number. A notebook could not compute it before this file existed, which left the
largest population of readers in this field able to run the analysis and unable to run the
argument.

Every test here reads a synthetic trace. They read a corpus trial until now, and every one of
them skipped wherever that trial is absent, which is every wheel this project ships: the
source distribution carries five of the workspace's crates and `plateforce-conformance` is
not among them, so `pytest {project}/crates/plateforce-python/tests` under cibuildwheel ran
eight skips and reported green. A skip is the one test result that cannot fail.

The question those eight could not answer, whether this surface's sweep is the terminal's, is
asked where it can be asked: `scripts/result-parity-requests.txt` names a `sweep` request, and
`scripts/result-parity.sh` holds this surface and the terminal to one committed record over
all 21 fields of a swept document on a real trial. That gate runs in a checkout, which is
where the trial is.

What a synthetic trace cannot carry, said once here rather than asserted weakly below. The
same sweep, the five published onset rules this build runs, spans 1.92 cm of jump height on
subject 01 trial 1 and 0.0038 cm on the trace `conftest.py` builds. So the magnitude is a
fact about real force data and no assertion here can hold it. It is held where real data is:
`tests/golden/result-parity-sweep.json` commits a wider sweep on that same trial, 75
combinations over all three landmark constructs spanning 3.11 cm, and the parity gate checks
it on every push.
"""

import numpy as np
import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ

# The three steps a jump height rests on, named here rather than defaulted inside the call,
# so a sweep over all three is asked for by this file rather than assumed.
LANDMARK_SLOTS = ["weighing", "onset", "takeoff"]

RULES = {
    "weighing": ("bwepoch.fixed_window", {"duration": 1.0}),
    "onset": ("onset.threshold.noise_relative", {"k": 5.0}),
    "takeoff": ("takeoff.threshold.absolute_force", {"threshold_n": 20.0}),
}

QUANTITY = "jump_height_from_takeoff_meters"


@pytest.fixture
def shipped_registry():
    """The registry this build embeds, which is the one a reader who pip installed has.

    Read rather than written, unlike the fixture registry the rest of the binding tests use:
    a sweep with no `method_ids` runs every rule the build's binding table holds for a slot,
    so the rules under test are the shipped ones either way and binding them from anywhere
    else would name ids the sweep does not reach.
    """
    return pf.Registry.load()


@pytest.fixture
def bound(shipped_registry):
    return {
        slot: shipped_registry.method(method_id).bind(**parameters)
        for slot, (method_id, parameters) in RULES.items()
    }


@pytest.fixture
def force_the_athlete_never_leaves(force_newtons):
    """The shared trace with its flight replaced by a settle back to standing.

    The shared trace ends in flight, so every combination swept over it places a takeoff and
    succeeds. A denominator asserted there is a denominator nothing was left out of, and the
    only trace on which a combination that produced nothing can be reached is one where the
    athlete stays on the plate.
    """
    flight_samples = int(0.5 * SAMPLE_RATE_HZ)
    on_the_plate = force_newtons[:-flight_samples]
    standing_newtons = float(on_the_plate[0])
    settling = np.linspace(float(on_the_plate[-1]), standing_newtons, flight_samples)
    return np.concatenate([on_the_plate, settling])


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


def test_a_sweep_that_left_on_its_own_says_what_produced_it(trial, bound, shipped_registry):
    """A spread nested in an analysis inherits that result's identity. One that leaves alone
    carries its own, or a reader holding it cannot say which software or which registry
    answered, which is the whole claim this package makes.

    All four fields, because three of them were here and `registry_declared_version` was not:
    this surface published the digest and the caller's pin and not what the registry says
    about itself, while its own analysed result published all three and so did every other
    surface's sweep."""
    swept = sweep(trial, bound, "onset")
    assert swept.plateforce_version == pf.__version__
    assert swept.registry_digest == shipped_registry.digest
    assert swept.registry_declared_version == shipped_registry.declared_version
    # Null because nobody pinned one, which is the question this field answers and not the
    # one above it. The two were transposed on two surfaces for weeks.
    assert swept.registry_version is None
    assert swept.registry_version != swept.registry_declared_version


def test_the_method_choice_moves_the_number(trial, bound):
    """Five published onset rules, five different numbers, on one trace.

    How far apart they land is a fact about the data and it is measured on real data, in the
    committed sweep record this file's header names. What is asserted here is that choosing
    between them is a choice at all: five rules returning one number would mean the sweep
    varied nothing, and a spread of zero reads identically to a sweep that never ran."""
    swept = sweep(trial, bound, "onset")
    assert swept.succeeded == 5
    assert swept.failed == 0
    assert len({v.value for v in swept.variants}) == 5, "five onset rules, one number"
    assert swept.spread_absolute > 0
    assert swept.baseline_value == pytest.approx(0.053501495638726144)


def test_a_variant_carries_what_it_bound(trial, bound):
    swept = sweep(trial, bound, "onset")
    labels = {v.label for v in swept.variants}
    assert "onset onset.threshold.noise_relative" in labels
    for variant in swept.variants:
        assert variant.settings["onset"] in variant.method_ids
        assert variant.failure_reason is None


def test_sweeping_a_parameter_holds_the_rule(trial, bound):
    swept = sweep(trial, bound, "onset", parameter="k", values=[1.0, 5.0, 10.0])
    assert swept.combinations_run == 3
    assert {v.settings["k"] for v in swept.variants} == {"1", "5", "10"}
    assert all(v.method_ids[1] == RULES["onset"][0] for v in swept.variants)


def test_the_denominator_holds_the_combinations_that_produced_nothing(trial, bound_methods):
    """A combination that produced nothing is counted, listed, and says why.

    Swept over a takeoff threshold no sample of this trace falls below, beside one several
    samples do, so the failing half is reached. Asserted over one combination on a trace that
    places every landmark, `failed` is 0, `combinations_run == succeeded + failed` is
    arithmetic rather than a claim about this software, and a build that dropped a failing
    combination from the count or from the list reads exactly like one that kept it.

    A threshold of zero newtons is the choice a reader makes who wants the instant force
    reaches nothing at all. The plate reads zero in flight and never less, so the crossing the
    rule looks for is one this recording does not contain.
    """
    epoch, onset, takeoff = bound_methods
    swept = pf.spread(
        trial,
        quantity=QUANTITY,
        slot="takeoff",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        parameter="threshold_n",
        values=[20.0, 0.0],
    )
    assert swept.failed > 0, "nothing failed, so every claim below is about arithmetic"
    assert swept.succeeded > 0, "nothing succeeded, so the failure is the sweep and not the value"
    assert swept.combinations_requested == 2
    assert swept.combinations_run == 2
    assert swept.combinations_run == swept.succeeded + swept.failed
    assert len(swept.variants) == swept.combinations_run

    produced_nothing = [variant for variant in swept.variants if variant.value is None]
    assert len(produced_nothing) == swept.failed
    for variant in produced_nothing:
        assert variant.failure_reason is not None, f"{variant.settings} produced nothing in silence"


def test_a_sweep_that_computed_nothing_reports_its_denominator_and_no_width(
    trial, force_the_athlete_never_leaves, bound_methods
):
    """Every combination failed, and the answer is three of three rather than an empty one.

    No width beside it. A spread of zero over a set where nothing computed reads as the choice
    of method moving the number by nothing, which is the reading this file's sibling refusal
    exists to prevent, and it is the reading a reader cannot tell from a real agreement.
    """
    epoch, onset, takeoff = bound_methods
    swept = pf.spread(
        pf.Trial(force_the_athlete_never_leaves, sample_rate_hz=SAMPLE_RATE_HZ),
        quantity=QUANTITY,
        slot="onset",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        parameter="k",
        values=[1.0, 5.0, 10.0],
    )
    assert swept.succeeded == 0
    assert swept.failed == 3
    assert swept.combinations_run == 3
    assert len(swept.variants) == 3
    assert swept.spread_absolute is None
    assert swept.spread_percent_of_median is None
    assert swept.baseline_value is None
    for variant in swept.variants:
        assert variant.value is None
        assert variant.failure_reason is not None, f"{variant.settings} produced nothing in silence"

    # The control, and it can come back empty for the same reason the assertions above can: the
    # identical sweep on the shared trace, which ends in flight. A build that failed every
    # combination everywhere satisfies all of the above and fails here.
    with_flight = pf.spread(
        trial,
        quantity=QUANTITY,
        slot="onset",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        parameter="k",
        values=[1.0, 5.0, 10.0],
    )
    assert with_flight.succeeded == 3
    assert with_flight.failed == 0


def test_a_slot_this_build_runs_no_rule_for_is_refused(trial, bound):
    with pytest.raises(pf.MethodError) as raised:
        sweep(trial, bound, "not_a_step")
    assert "not_a_step" in str(raised.value)


def test_a_step_the_table_holds_one_rule_for_is_refused_in_the_terminals_words(trial, bound):
    """The terminal refuses `--slot time_to_takeoff`; this call ran it and reported zero.

    A step with one rule has nothing for that rule to be compared against, so the sweep ran a
    single variant and reported a spread of 0.0, which reads as the choice of method moving
    the number by nothing. That is the reading this software exists to prevent, and it was
    reachable from a notebook on six of the binding table's steps and from no terminal.

    `time_to_takeoff` is named rather than searched for, so this says which step it asks
    about. A second rule filed under that construct turns this red rather than quiet, which is
    the right way round: the step stops being an example and the test has to say so.

    The sentence is the terminal's, asserted whole rather than by a fragment of it, because
    the point is that a reader meets one wording whichever keyboard they are at. The
    terminal's half of the pair is `a_step_with_one_rule_is_refused_rather_than_dropped...`
    in `crates/plateforce-cli/tests/spread.rs`, which asserts these same words.
    """
    with pytest.raises(pf.MethodError) as raised:
        sweep(trial, bound, "time_to_takeoff")
    assert str(raised.value) == (
        "this analysis runs one rule for time_to_takeoff, so there is nothing to sweep"
    )

    # The control, and the half that says the floor is a floor rather than a wall: the five
    # onset rules are still swept, so a refusal that refused everything would not pass here.
    still_runs = sweep(trial, bound, "onset")
    assert still_runs.combinations_run == 5
    assert still_runs.spread_absolute > 0


def test_a_step_named_twice_is_one_axis_rather_than_a_sweep_squared(trial, bound):
    """One step is one axis, in the terminal's words.

    Named twice it was two axes: the same five onset rules ran 25 combinations, every one of
    them binding onset twice with the second binding winning, so twenty of the twenty-five
    repeated a rule and the denominator each figure was reported over counted a set nobody
    asked for. The terminal has refused this since the flag existed.
    """
    with pytest.raises(pf.MethodError) as raised:
        sweep(trial, bound, ["onset", "onset"])
    assert str(raised.value) == "'onset' is named twice, and one step is one axis"

    # Two different steps are two axes, which is the whole point of taking a list.
    both = sweep(trial, bound, ["onset", "takeoff"])
    assert both.combinations_run == 25


def test_a_parameter_and_several_slots_are_refused_together(trial, bound):
    """Each describes one slot, so the pair states a sweep nobody can mean."""
    with pytest.raises(pf.MethodError):
        sweep(trial, bound, LANDMARK_SLOTS, parameter="k", values=[1.0, 5.0])


def test_naming_no_slot_is_refused(trial, bound):
    with pytest.raises(pf.MethodError):
        sweep(trial, bound, [])


def test_the_cap_is_reported_rather_than_applied_silently(trial, bound):
    swept = sweep(trial, bound, LANDMARK_SLOTS, maximum_combinations=4)
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
