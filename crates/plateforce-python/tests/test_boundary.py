"""Which objects the library reads back in, and which it refuses at the door.

An `Acquisition` and a `Sentinel` are read out of the Python object and kept. Every other
class in this package travels one way. A look-alike carrying the right attribute names is
refused rather than read: `acquisition_complete` is what permits a result to be set beside
another lab's, and a sentinel convention decides which samples were never measurements.
Neither may be taken from an object this library did not build.
"""

import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ


class AcquisitionLookAlike:
    """Every attribute an `Acquisition` carries, and none of its type."""

    filter_at_capture = "none"
    tare_state = "tared_before_trial"
    plate_natural_frequency_hz = 800.0
    floor_surface = "concrete"
    firmware_version = "impostor-0"
    is_complete = True
    missing = []


class SentinelLookAlike:
    name = "zero"


def test_an_acquisition_arrives_with_its_fields_intact(force_newtons, complete_acquisition):
    trial = pf.Trial(
        force_newtons, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=complete_acquisition
    )
    assert trial.acquisition_complete
    assert trial.acquisition.firmware_version == "synthetic-0"
    assert trial.acquisition.plate_natural_frequency_hz == 800.0


def test_a_sentinel_arrives_at_both_entry_points_as_the_one_declared():
    values = [45.0, -999.0, 51.0]

    trial = pf.Trial(values, sample_rate_hz=100.0, sentinel=pf.Sentinel.value(-999.0))
    assert trial.exclusions.sentinel_convention == "value(-999)"
    assert trial.exclusions.dropped_samples == 1

    partition = pf.partition_sentinel_values(values, pf.Sentinel.value(-999.0))
    assert partition.exclusions.sentinel_convention == "value(-999)"
    assert partition.kept == [45.0, 51.0]


def test_the_acquisition_slot_reads_the_class_and_not_the_attribute_names(force_newtons):
    for impostor in (AcquisitionLookAlike(), {"tare_state": "tared_before_trial"}, "complete"):
        with pytest.raises(TypeError):
            pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=impostor)


def test_a_sentinel_convention_is_named_rather_than_inferred_from_a_number():
    values = [45.0, 0.0, 51.0]
    with pytest.raises(TypeError):
        pf.Trial(values, sample_rate_hz=100.0, sentinel=0.0)
    with pytest.raises(TypeError):
        pf.partition_sentinel_values(values, 0.0)
    with pytest.raises(TypeError):
        pf.partition_sentinel_values(values, SentinelLookAlike())


def test_the_acquisition_slot_refuses_the_other_classes_this_package_hands_out(
    force_newtons, bound_methods
):
    weighing_epoch, _, _ = bound_methods
    for handed_out in (pf.Sentinel.zero(), weighing_epoch, weighing_epoch.entry):
        with pytest.raises(TypeError):
            pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=handed_out)


def test_an_unbound_entry_is_refused_where_a_bound_method_belongs(
    trial, registry, bound_methods
):
    """Binding is where the parameter values are fixed, and the provenance quotes them."""
    _, onset, takeoff = bound_methods
    with pytest.raises(TypeError):
        pf.analyse_countermovement_jump(
            trial, registry.method("bwepoch.fixed_window"), onset, takeoff
        )


def test_a_trial_slot_and_a_method_slot_each_take_only_their_own_class(
    trial, bound_methods, complete_acquisition
):
    weighing_epoch, onset, takeoff = bound_methods
    with pytest.raises(TypeError):
        pf.analyse_countermovement_jump(trial, complete_acquisition, onset, takeoff)
    with pytest.raises(TypeError):
        pf.analyse_countermovement_jump(weighing_epoch, weighing_epoch, onset, takeoff)
