"""A failure has to say which method and which parameter.

The core error text already names both. These tests check it arrives that way rather than
flattened into something a user cannot act on.
"""

import numpy as np
import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ


def test_a_rule_that_finds_nothing_names_the_method_and_the_parameter(registry):
    quiet = np.full(2400, 600.0, dtype=np.float64)
    quiet[::7] += 0.5  # a noise floor, so the band has a non-zero width to not cross
    trial = pf.Trial(quiet, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.NoCrossingError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)

    message = str(raised.value)
    assert "onset.threshold.noise_relative" in message
    assert "k = 5" in message
    assert "no crossing" in message


def test_the_failure_carries_its_fields_so_they_need_not_be_parsed_out(registry):
    quiet = np.full(2400, 600.0, dtype=np.float64)
    quiet[::7] += 0.5
    trial = pf.Trial(quiet, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.NoCrossingError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)

    assert raised.value.method_id == "onset.threshold.noise_relative"
    assert raised.value.parameter == "k"
    assert raised.value.value == pytest.approx(5.0)
    assert raised.value.search_bound_seconds > 0.0


def test_a_takeoff_that_never_happens_names_the_takeoff_rule(registry, force_newtons):
    trial = pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    # No sample in the trace falls below a negative threshold.
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=-5.0)

    with pytest.raises(pf.NoCrossingError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)

    assert raised.value.method_id.startswith("takeoff.threshold.absolute")
    assert raised.value.parameter == "threshold_newtons"


@pytest.mark.xfail(
    strict=True,
    reason=(
        "plateforce-core reports 'takeoff.threshold.absolute' and the registry entry is "
        "'takeoff.threshold.absolute_force', so the id in the error does not resolve. "
        "Delete this marker once the two agree."
    ),
)
def test_the_id_in_a_failure_resolves_in_the_registry(registry, force_newtons):
    """An error names a method so the reader can go and look it up.

    An id that resolves nowhere is the defect this project documents, wearing our badge.
    """
    trial = pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=-5.0)

    with pytest.raises(pf.NoCrossingError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)

    assert raised.value.method_id in set(registry.method_ids())


def test_a_weighing_epoch_longer_than_the_trace_says_both_durations(registry):
    trial = pf.Trial(np.full(100, 600.0, dtype=np.float64), sample_rate_hz=100.0)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=10.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.TrialError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)

    message = str(raised.value)
    assert "10" in message
    assert "does not fit" in message


def test_selecting_a_described_but_unimplemented_entry_fails_by_name(registry, trial):
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    unimplemented = registry.method("onset.threshold.noise_relative_forward_offset").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.MethodNotImplementedError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, unimplemented, takeoff)

    message = str(raised.value)
    assert "onset.threshold.noise_relative_forward_offset" in message
    assert "onset.threshold.noise_relative" in message


def test_a_method_passed_in_the_wrong_slot_is_refused(registry, trial):
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    with pytest.raises(pf.MethodNotImplementedError):
        pf.analyse_countermovement_jump(trial, epoch, onset, onset)


def test_an_unknown_enumerated_choice_lists_what_is_accepted(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    with pytest.raises(ValueError) as raised:
        pf.analyse_countermovement_jump(
            trial, epoch, onset, takeoff, dispersion_estimator="numpy_default"
        )
    message = str(raised.value)
    assert "dispersion_estimator" in message
    assert "population" in message
    assert "sample" in message


def test_every_error_shares_one_base_so_a_caller_can_catch_the_package(registry):
    for error in (
        pf.TrialError,
        pf.NoCrossingError,
        pf.RegistryError,
        pf.MethodError,
        pf.ParameterError,
        pf.MethodNotImplementedError,
    ):
        assert issubclass(error, pf.PlateforceError)
    assert issubclass(pf.NoCrossingError, pf.TrialError)
    assert issubclass(pf.ParameterError, pf.MethodError)
    assert issubclass(pf.MethodNotImplementedError, pf.MethodError)
