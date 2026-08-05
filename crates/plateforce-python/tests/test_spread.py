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


def test_sweeping_a_setting_holds_the_rule(trial, bound):
    swept = sweep(trial, bound, None, vary={"onset.k": [1.0, 5.0, 10.0]})
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


def test_the_rules_and_a_setting_inside_them_vary_on_one_call(trial, bound):
    """The engine sweeps a mixed set of axes and no surface could state one.

    `k` moves a jump height as far as the choice of onset rule does, so a reader holding a
    figure that rests on both is asking one question, and asking them separately reports
    neither the widest disagreement nor the narrowest. The terminal writes the same request as
    `--slot onset --vary onset.k=2,5,10`."""
    over_the_rules = sweep(trial, bound, "onset")
    over_a_value = sweep(trial, bound, None, vary={"onset.k": [1.0, 5.0, 10.0]})
    over_both = sweep(trial, bound, "onset", vary={"onset.k": [1.0, 5.0, 10.0]})

    assert over_the_rules.combinations_run > 1 and over_a_value.combinations_run > 1
    assert (
        over_both.combinations_run
        == over_the_rules.combinations_run * over_a_value.combinations_run
    )
    # The record names both, so a reader of the figure can see the whole set it came from.
    assert len(over_both.variants) == over_both.combinations_run
    assert over_both.spread_absolute >= max(
        over_the_rules.spread_absolute, over_a_value.spread_absolute
    )


def test_a_setting_named_twice_is_one_axis(trial, bound):
    with pytest.raises(pf.MethodError) as raised:
        sweep(trial, bound, None, vary={"onset.k": [5.0, 5.0]})
    assert str(raised.value) == "onset.k names 5 twice, and one value is one variant"


def test_a_list_of_method_ids_describes_one_step(trial, bound, shipped_registry):
    """As a list the ids cannot say which step each belongs to. Keyed by step they can, so
    named rules on two steps is one call."""
    with pytest.raises(pf.MethodError) as raised:
        sweep(trial, bound, LANDMARK_SLOTS, method_ids=["onset.threshold.noise_relative"])
    assert "describes one step" in str(raised.value)

    # Two ids per step, taken from the sweep that compares every rule the build runs for it,
    # so the pair is read off this build rather than written here.
    onset_rules = [v.settings["onset"] for v in sweep(trial, bound, "onset").variants][:2]
    takeoff_rules = [v.settings["takeoff"] for v in sweep(trial, bound, "takeoff").variants][:2]
    keyed = sweep(
        trial,
        bound,
        None,
        method_ids={"onset": onset_rules, "takeoff": takeoff_rules},
    )
    assert keyed.combinations_run == 4
    assert len(keyed.axes_varied) == 2


def test_a_name_a_rule_takes_is_swept_the_way_its_numbers_are(trial, bound, shipped_registry):
    """An enumerated setting is a setting. Net against gross impulse over one epoch differ by
    the system weight across it, and no surface could compare the two names."""
    swept = pf.spread(
        trial,
        quantity="epoch_impulse_newton_seconds",
        vary={"epoch_impulse.convention": ["net", "gross"]},
        weighing_epoch=bound["weighing"],
        onset=bound["onset"],
        takeoff=bound["takeoff"],
        derived={"epoch_impulse": shipped_registry.method("impulse.epoch_from_onset").bind()},
        derived_options={"epoch_impulse": {"convention": "net"}},
    )
    assert swept.combinations_run == 2
    assert swept.succeeded == 2
    assert swept.spread_absolute > 0, "the names did not reach the rule"
    assert [v.settings["convention"] for v in swept.variants] == ["gross", "net"]

    # Numbers and names on one axis have no width between them, so the pair is refused.
    with pytest.raises(pf.PlateforceError):
        pf.spread(
            trial,
            quantity="epoch_impulse_newton_seconds",
            vary={"epoch_impulse.convention": ["net", 5.0]},
            weighing_epoch=bound["weighing"],
            onset=bound["onset"],
            takeoff=bound["takeoff"],
        )


def test_the_sweep_states_every_argument_the_analysis_states(trial, bound, shipped_registry):
    """A sweep varies the request an analysis sends, so the two take one argument set.

    Thirteen of the analysis arguments reached the builder as `None` written thirteen times,
    and a notebook could sweep around no derived construct, no conditioning rule, no placed
    landmark and no name a rule reads. The assertion is that each of them reaches the request:
    a derived rule the sweep held is on `held_fixed`, which is the record of what stood still.
    """
    swept = pf.spread(
        trial,
        quantity=QUANTITY,
        slot="onset",
        weighing_epoch=bound["weighing"],
        onset=bound["onset"],
        takeoff=bound["takeoff"],
        derived={"epoch_impulse": shipped_registry.method("impulse.epoch_from_onset").bind()},
        derived_options={"epoch_impulse": {"convention": "net"}},
        derived_parameters={"epoch_impulse": {"epoch_ms": 150.0}},
    )
    assert swept.succeeded, "the sweep produced no combination, so it proves nothing"

    # The control: the same call without the derived rule does not carry it, so the assertion
    # below is about the argument reaching the request rather than about a rule that runs
    # whether or not anybody named one.
    without = sweep(trial, bound, "onset")
    assert "epoch_impulse" not in _held(without)
    assert "epoch_impulse" in _held(swept)


def _held(swept):
    """Which constructs a sweep says stood still, read off its own record."""
    return set(swept.held_fixed)


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
