"""A failure has to say which method and which parameter.

The core error text already names both. These tests check it arrives that way rather than
flattened into something a user cannot act on.
"""

import numpy as np
import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ


def quiet_partial_result(registry):
    quiet = np.full(2400, 600.0, dtype=np.float64)
    quiet[::7] += 0.5  # a noise floor, so the band has a non-zero width to not cross
    trial = pf.Trial(quiet, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)
    return pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)


def test_landmark_refusals_leave_the_usable_quantities_in_place(registry):
    result = quiet_partial_result(registry)

    assert result.system_weight_newtons.value == pytest.approx(600.0716666667)
    assert result.system_mass_kilograms is not None
    assert result.onset_index is None
    assert result.takeoff_index is None
    assert result.onset_time_seconds is None
    assert result.takeoff_time_seconds is None


def test_all_landmark_refusals_leave_together(registry):
    result = quiet_partial_result(registry)

    assert len(result.refusals) == 2, "returned 2 of 2 landmark refusals"
    assert {refusal.slot for refusal in result.refusals} == {"movement_onset", "takeoff"}


def test_a_rule_that_finds_nothing_names_the_method_and_the_parameter(registry):
    result = quiet_partial_result(registry)
    refused = next(
        refusal
        for refusal in result.refusals
        if refusal.method_id == "onset.threshold.noise_relative"
    )

    message = str(refused)
    assert "onset.threshold.noise_relative" in message
    assert "k = 5" in message
    assert "no crossing" in message


def test_the_failure_carries_its_fields_so_they_need_not_be_parsed_out(registry):
    result = quiet_partial_result(registry)
    refused = next(
        refusal
        for refusal in result.refusals
        if refusal.method_id == "onset.threshold.noise_relative"
    )

    assert isinstance(refused, pf.NoCrossingError)
    assert refused.method_id == "onset.threshold.noise_relative"
    assert refused.parameter == "k"
    assert refused.value == pytest.approx(5.0)
    assert refused.search_bound_seconds > 0.0


def test_a_takeoff_that_never_happens_names_the_takeoff_rule(registry, force_newtons):
    trial = pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    # No sample in the trace falls below a negative threshold.
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=-5.0)

    result = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    refused = next(
        refusal
        for refusal in result.refusals
        if refusal.method_id == "takeoff.threshold.absolute_force"
    )

    assert refused.method_id == "takeoff.threshold.absolute_force"
    assert refused.parameter == "threshold_newtons"


def test_the_id_in_a_failure_resolves_in_the_registry(registry, force_newtons):
    """An error names a method so the reader can go and look it up.

    An id that resolves nowhere is the defect this project documents, wearing our badge.
    """
    trial = pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=-5.0)

    result = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    refused = next(
        refusal
        for refusal in result.refusals
        if refusal.method_id == "takeoff.threshold.absolute_force"
    )

    assert refused.method_id in set(registry.method_ids())


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
    """A value the rule does not take is refused, not mapped onto the nearest one. A
    substitution would put the word the caller wrote next to a number a different rule
    produced."""
    epoch, onset, takeoff = bound_methods
    with pytest.raises(pf.PlateforceError) as raised:
        pf.analyse_countermovement_jump(
            trial, epoch, onset, takeoff, weighing_options={"dispersion": "numpy_default"}
        )
    message = str(raised.value)
    assert "dispersion" in message
    assert "numpy_default" in message
    assert "population" in message
    assert "sample" in message


def test_a_refusal_carries_the_code_the_manifest_publishes(registry, force_newtons):
    """The code is a field, so a caller branches on it rather than on the exception class or
    on a substring of the sentence. R raises the same word for the same failure, which is
    what makes a refusal one thing across the two languages rather than two."""
    trial = pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    onset = registry.method("onset.threshold.noise_relative").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=-5.0)

    result = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    refused = next(
        refusal
        for refusal in result.refusals
        if refusal.method_id == "takeoff.threshold.absolute_force"
    )

    assert refused.code == "no_crossing"
    assert refused.slot == "takeoff"
    # Everything the rule read, as the mapping an R condition carries under the same name,
    # and as an attribute for a caller who wants the number without a lookup.
    assert refused.detail["search_bound_seconds"] > 0.0
    assert refused.search_bound_seconds == refused.detail["search_bound_seconds"]


def test_selecting_an_unbound_rule_raises_the_code_rather_than_a_sentence(registry, trial):
    """An analysis that ends before any number computes used to reach Python as a
    MethodNotImplementedError carrying a sentence and no code at all."""
    epoch = registry.method("bwepoch.fixed_window").bind(duration=1.0)
    unimplemented = registry.method("onset.threshold.noise_relative_forward_offset").bind(k=5.0)
    takeoff = registry.method("takeoff.threshold.absolute_force").bind(threshold_n=20.0)

    with pytest.raises(pf.MethodNotImplementedError) as raised:
        pf.analyse_countermovement_jump(trial, epoch, unimplemented, takeoff)

    assert raised.value.code == "method_not_implemented"
    assert raised.value.method_id == "onset.threshold.noise_relative_forward_offset"
    # The construct as the registry names it, which is the spelling R carries too.
    assert raised.value.slot == "movement_onset"
    assert len(raised.value.available) > 0


def test_every_error_shares_one_base_so_a_caller_can_catch_the_package(registry):
    for error in (
        pf.TrialError,
        pf.NoCrossingError,
        pf.CollapsedBandError,
        pf.RegistryError,
        pf.MethodError,
        pf.ParameterError,
        pf.MethodNotImplementedError,
    ):
        assert issubclass(error, pf.PlateforceError)
    assert issubclass(pf.NoCrossingError, pf.TrialError)
    assert issubclass(pf.CollapsedBandError, pf.TrialError)
    assert issubclass(pf.ParameterError, pf.MethodError)
    assert issubclass(pf.MethodNotImplementedError, pf.MethodError)
