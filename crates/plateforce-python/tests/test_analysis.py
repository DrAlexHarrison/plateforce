"""The analysis surface, and the identities the core guarantees reaching Python unchanged."""

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
        trial, epoch, onset, takeoff, dispersion_estimator="population"
    )
    sample = pf.analyse_countermovement_jump(
        trial, epoch, onset, takeoff, dispersion_estimator="sample"
    )
    assert (
        population.onset_time_seconds.provenance.enumerated_choices["dispersion_estimator"]
        == "population"
    )
    assert (
        sample.onset_time_seconds.provenance.enumerated_choices["dispersion_estimator"] == "sample"
    )


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
        onset_degenerate_band="fraction_of_reference",
        onset_degenerate_band_fraction=0.95,
    )
    assert (
        rescued.onset_time_seconds.provenance.enumerated_choices["degenerate_band"]
        == "fraction_of_reference"
    )


def test_the_fraction_policy_needs_its_fraction(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    with pytest.raises(ValueError) as raised:
        pf.analyse_countermovement_jump(
            trial, epoch, onset, takeoff, onset_degenerate_band="fraction_of_reference"
        )
    assert "onset_degenerate_band_fraction" in str(raised.value)


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
