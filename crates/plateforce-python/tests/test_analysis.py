"""The analysis surface, and the identities the core guarantees reaching Python unchanged."""

import json

import numpy as np
import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ, STANDARD_GRAVITY, SYSTEM_MASS_KILOGRAMS


@pytest.fixture
def jump(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    return pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)


def test_the_weighing_epoch_recovers_the_system_mass(jump):
    assert jump.system_mass_kilograms.value == pytest.approx(SYSTEM_MASS_KILOGRAMS, abs=0.05)
    assert jump.system_weight_newtons.value == pytest.approx(
        SYSTEM_MASS_KILOGRAMS * STANDARD_GRAVITY, abs=0.5
    )


def test_a_fixed_window_ties_with_nothing(jump):
    assert jump.weighing_epoch_tied_window_count == 1


def test_the_landmarks_are_ordered_and_inside_the_trace(jump, trial):
    assert 0 < jump.onset_index < jump.takeoff_index < trial.sample_count
    assert jump.onset_time_seconds.value == pytest.approx(trial.time_at(jump.onset_index))
    assert jump.takeoff_time_seconds.value == pytest.approx(trial.time_at(jump.takeoff_index))


def test_time_to_takeoff_spans_the_two_landmarks(jump):
    expected = (jump.takeoff_index - jump.onset_index) / SAMPLE_RATE_HZ
    assert jump.time_to_takeoff_seconds.value == pytest.approx(expected)


def test_jump_height_follows_takeoff_velocity(jump):
    velocity = jump.takeoff_velocity_meters_per_second.value
    expected = velocity**2 / (2.0 * STANDARD_GRAVITY)
    assert jump.jump_height_takeoff_frame_meters.value == pytest.approx(expected)


def test_reactive_strength_index_is_height_over_time_to_takeoff(jump):
    expected = (
        jump.jump_height_takeoff_frame_meters.value / jump.time_to_takeoff_seconds.value
    )
    assert jump.reactive_strength_index_modified.value == pytest.approx(expected)


def test_the_onset_rule_moves_the_answer_which_is_the_whole_point(trial, registry):
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)
    entry = registry.method("onset.threshold.noise_relative")

    at_two = pf.analyse_countermovement_jump(trial, epoch, entry.bind(k=2.0), takeoff)
    at_ten = pf.analyse_countermovement_jump(trial, epoch, entry.bind(k=10.0), takeoff)

    assert at_two.onset_index != at_ten.onset_index
    assert at_two.time_to_takeoff_seconds.value != at_ten.time_to_takeoff_seconds.value

    # The k that moved the number sits on the upstream onset step, not on this one.
    onset_id = "onset.threshold.noise_relative"
    assert at_two.time_to_takeoff_seconds.provenance.parameters_of(onset_id)["k"] == 2.0
    assert at_ten.time_to_takeoff_seconds.provenance.parameters_of(onset_id)["k"] == 10.0


def test_the_weighing_window_moves_the_answer_too(trial, registry):
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)
    entry = registry.method("bwepoch.fixed_window")

    short = pf.analyse_countermovement_jump(trial, entry.bind(duration=0.5), onset, takeoff)
    long = pf.analyse_countermovement_jump(trial, entry.bind(duration=1.0), onset, takeoff)
    assert short.system_weight_newtons.value != long.system_weight_newtons.value


def test_the_dispersion_estimator_is_a_real_choice(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    population = pf.analyse_countermovement_jump(
        trial, epoch, onset, takeoff, weighing_options={"dispersion": "population"}
    )
    sample = pf.analyse_countermovement_jump(
        trial, epoch, onset, takeoff, weighing_options={"dispersion": "sample"}
    )
    assert (
        population.system_weight_newtons.provenance.enumerated_choices["dispersion"]
        == "population"
    )
    assert sample.system_weight_newtons.provenance.enumerated_choices["dispersion"] == "sample"
    # The onset threshold is scaled by the weighing window's spread, so the onset row
    # records the convention that window was computed under rather than one of its own.
    assert sample.onset_time_seconds.provenance.enumerated_choices["sd_convention"] == "sample"


def test_a_collapsed_noise_band_is_refused_rather_than_substituted(registry):
    """A perfectly still window gives k times zero, which separates nothing.

    Refusing is the default because a silent fallback would hide that the window the rule
    assumed was quiet was not a real weighing epoch.
    """
    force = np.concatenate(
        [np.full(1200, 600.0), np.linspace(600.0, 0.0, 600), np.zeros(600)]
    )
    trial = pf.Trial(force, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.CollapsedBandError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    assert raised.value.method_id == "onset.threshold.noise_relative"
    assert raised.value.parameter == "k"
    assert raised.value.dispersion_newtons == 0.0
    assert "no band to search" in str(raised.value)

    rescued = pf.analyse_countermovement_jump(
        trial,
        epoch,
        onset,
        takeoff,
        onset_parameters={"degenerate_fraction": 0.95},
    )
    assert (
        rescued.onset_time_seconds.provenance.bound_parameters["degenerate_fraction"]
        == 0.95
    )


def test_the_replacement_band_is_a_fraction_and_not_a_policy_name(trial, bound_methods):
    """The fraction is the choice. A policy name with no fraction behind it named a
    behaviour the rule could not carry out."""
    epoch, onset, takeoff = bound_methods
    jump = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    assert (
        jump.onset_time_seconds.provenance.enumerated_choices["degenerate_band"] == "refuse"
    )


def test_the_same_input_and_the_same_choices_give_the_same_answer(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    first = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    second = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    assert (
        first.jump_height_takeoff_frame_meters.value
        == second.jump_height_takeoff_frame_meters.value
    )


def test_a_numpy_trace_and_a_list_trace_give_the_same_answer(force_newtons, bound_methods):
    epoch, onset, takeoff = bound_methods
    from_numpy = pf.analyse_countermovement_jump(
        pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ), epoch, onset, takeoff
    )
    from_list = pf.analyse_countermovement_jump(
        pf.Trial(list(force_newtons), sample_rate_hz=SAMPLE_RATE_HZ), epoch, onset, takeoff
    )
    assert (
        from_numpy.jump_height_takeoff_frame_meters.value
        == from_list.jump_height_takeoff_frame_meters.value
    )


def test_sentinels_declared_on_the_trial_are_reported_on_the_result(
    force_newtons, bound_methods
):
    epoch, onset, takeoff = bound_methods
    trial = pf.Trial(
        force_newtons, sample_rate_hz=SAMPLE_RATE_HZ, sentinel=pf.Sentinel.zero()
    )
    jump = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    assert jump.trial_exclusions.sentinel_convention == "zero"
    assert jump.trial_exclusions.dropped_samples == 600, "the flight phase reads exactly zero here"


def test_the_module_states_its_version():
    assert pf.__version__


def test_a_rule_named_by_its_construct_reports_its_number_and_the_rule_behind_it(
    registry, trial
):
    """A construct other than the three the spine walks, asked for by name.

    Before the engine could dispatch by construct id no surface could ask for one at all,
    and this surface was the last of the four to be able to.
    """
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    result = pf.analyse_countermovement_jump(
        trial,
        epoch,
        onset,
        takeoff,
        derived={
            "analysis_window": registry.method("window_end.takeoff.detected").bind(),
            "peak_force": registry.method("force.peak.gross").bind(),
        },
    )
    peak = result.value("peak_force_newtons")
    assert peak.value > 0
    assert peak.unit == "newtons"
    assert peak.provenance.method_id == "force.peak.gross"


def test_a_value_stated_against_a_construct_reaches_its_rule(registry, trial):
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    def peak_at(width):
        result = pf.analyse_countermovement_jump(
            trial,
            epoch,
            onset,
            takeoff,
            derived={
                "analysis_window": registry.method("window_end.takeoff.detected").bind(),
                "peak_force": registry.method("force.peak.estimator").bind(),
            },
            derived_parameters={"peak_force": {"averaging_window_seconds": width}},
        )
        return result.value("peak_force_newtons").value

    assert peak_at(0.1) < peak_at(0.0)


def test_a_construct_this_build_runs_no_rule_for_is_refused_by_name(registry, trial):
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.MethodNotImplementedError) as refused:
        pf.analyse_countermovement_jump(
            trial,
            epoch,
            onset,
            takeoff,
            derived={"movement_onset": registry.method("force.peak.gross").bind()},
        )
    assert "movement_onset" in str(refused.value)
    assert "peak_force" in str(refused.value)


def test_a_named_choice_stated_for_a_derived_rule_is_recorded_as_the_callers_own(trial):
    """One trial can state what a folder run has always been able to state.

    `pf.batch` took `derived_options` and this call did not, so a construct computed from the
    landmarks whose rule turns on a choice between named alternatives ran under whatever the
    registry binds when nobody chooses, whichever way the caller wanted it. Worse than the
    number: the record said the choice was assumed while the caller was holding it.

    Both halves on one rule and one run, because a build that answered "stated" for everything
    passes the first assertion and fails the second, and one that answered "assumed" for
    everything fails the first. The braking-phase rule reads `search_signal`, and the two
    values reach two different searches, so this is a choice that moves the number rather than
    a label.

    The shipped registry rather than the fixture one, because the fixture registry carries
    five constructs and none of their rules declares an enumeration.
    """
    shipped = pf.Registry.load()
    weighing = shipped.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = shipped.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = shipped.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)
    braking = shipped.method("phase.braking_start.zero_net_force").bind()

    def analysed(options):
        return pf.analyse_countermovement_jump(
            trial,
            weighing,
            onset,
            takeoff,
            derived={"braking_phase_start": braking},
            derived_options=options,
        )

    def chosen_by(result):
        steps = result.value("braking_phase_start_seconds").provenance.flattened()
        recorded = [
            dict(step.enumerated_choices)
            for step in steps
            if step.method_id == "phase.braking_start.zero_net_force"
        ]
        assert len(recorded) == 1, recorded
        return recorded[0]["search_signal"]

    def source_in_the_engines_own_record(options):
        """`parameter_sources` as the engine wrote it, keyed by the rule that read the name.

        `assumed_parameters` above is one flat list over every rule on the path, and two rules
        in this registry declare a `search_signal`. This reads the record of the one rule.
        """
        document = json.loads(
            pf._analyse_json(
                trial,
                weighing_epoch=weighing,
                onset=onset,
                takeoff=takeoff,
                derived={"braking_phase_start": braking},
                derived_options=options,
            )
        )
        bound = [
            row
            for row in document["ok"]["bound_methods"]
            if row["method_id"] == "phase.braking_start.zero_net_force"
        ]
        assert len(bound) == 1, bound
        return bound[0]["parameter_sources"]["search_signal"]

    choice = {"braking_phase_start": {"search_signal": "force_bw_crossing"}}
    stated = analysed(choice)
    assert chosen_by(stated) == "force_bw_crossing"
    assert "search_signal" not in stated.assumed_parameters
    assert source_in_the_engines_own_record(choice) == "stated"

    # The control. Nobody chooses, so the registry's own value is bound and the record says
    # it was assumed, which is the distinction the whole product rests on.
    assumed = analysed(None)
    assert chosen_by(assumed) == "velocity_argmin"
    assert "search_signal" in assumed.assumed_parameters
    assert source_in_the_engines_own_record(None) == "assumed"
