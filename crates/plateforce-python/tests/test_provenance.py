"""What survives the boundary with a number.

A result crossing into Python keeps the method that produced it. These tests are the ones
that fail if it ever becomes a bare float.
"""

import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ


@pytest.fixture
def jump(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    return pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)


def test_a_result_is_not_a_float_and_will_not_pretend_to_be_one(jump):
    height = jump.jump_height_takeoff_frame_meters
    assert isinstance(height, pf.Measured)
    assert not isinstance(height, float)
    with pytest.raises(TypeError):
        float(height)
    with pytest.raises(TypeError):
        height + 1
    with pytest.raises(TypeError):
        height * 2


def test_the_bare_number_is_reachable_but_only_by_asking(jump):
    height = jump.jump_height_takeoff_frame_meters
    assert isinstance(height.value, float)
    assert height.unit == "meters"


def test_every_result_names_its_method_and_unit(jump):
    expected = {
        "system_weight_newtons": ("newtons", "bwepoch.fixed_window"),
        "onset_time_seconds": ("seconds", "onset.threshold.noise_relative"),
        "takeoff_time_seconds": ("seconds", "takeoff.threshold.absolute_force"),
        "time_to_takeoff_seconds": ("seconds", "time_to_takeoff.onset_to_takeoff"),
        "jump_height_takeoff_frame_meters": ("meters", "jump_height.from_takeoff_velocity"),
    }
    for attribute, (unit, method_id) in expected.items():
        result = getattr(jump, attribute)
        assert result.unit == unit, attribute
        assert result.provenance.method_id == method_id, attribute


def test_the_bound_parameters_travel_with_the_number(jump):
    onset = jump.onset_time_seconds.provenance
    assert onset.bound_parameters == {"k": 5.0}
    epoch = jump.system_weight_newtons.provenance
    assert epoch.bound_parameters == {"duration": 1.0}


def test_the_registry_version_travels_with_the_number(jump):
    assert jump.jump_height_takeoff_frame_meters.provenance.registry_version == "fixture-1"


def test_choices_that_are_not_numbers_travel_too(jump):
    onset = jump.onset_time_seconds.provenance
    assert onset.enumerated_choices["band_sides"] == "below_only"
    assert onset.enumerated_choices["dispersion_estimator"] == "sample"


def test_jump_height_names_the_upstream_choices_that_moved_it(jump):
    chain = jump.jump_height_takeoff_frame_meters.provenance
    reached = set()

    def walk(provenance):
        reached.add(provenance.method_id)
        for parent in provenance.depends_on:
            walk(parent)

    walk(chain)
    assert "onset.threshold.noise_relative" in reached
    assert "takeoff.threshold.absolute_force" in reached
    assert "bwepoch.fixed_window" in reached, "the weighing epoch moves jump height too"


def test_describe_shows_the_value_and_the_whole_chain(jump):
    described = jump.jump_height_takeoff_frame_meters.describe()
    assert "meters" in described
    assert "jump_height.from_takeoff_velocity" in described
    assert "onset.threshold.noise_relative" in described
    assert "bwepoch.fixed_window" in described
    assert "registry fixture-1" in described


def test_an_incomplete_acquisition_block_is_stated_on_every_result(jump):
    assert jump.jump_height_takeoff_frame_meters.provenance.acquisition_complete is False
    assert "acquisition block incomplete" in jump.jump_height_takeoff_frame_meters.describe()


def test_a_complete_acquisition_block_clears_the_flag(
    force_newtons, bound_methods, complete_acquisition
):
    epoch, onset, takeoff = bound_methods
    trial = pf.Trial(
        force_newtons, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=complete_acquisition
    )
    complete = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    height = complete.jump_height_takeoff_frame_meters
    assert height.provenance.acquisition_complete is True
    assert "acquisition block incomplete" not in height.describe()


def test_gravity_is_recorded_because_the_tools_disagree_on_it(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    standard = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    common = pf.analyse_countermovement_jump(
        trial, epoch, onset, takeoff, gravity_meters_per_second_squared=9.81
    )
    assert standard.jump_height_takeoff_frame_meters.provenance.bound_parameters[
        "gravity_meters_per_second_squared"
    ] == pytest.approx(9.80665)
    assert common.jump_height_takeoff_frame_meters.provenance.bound_parameters[
        "gravity_meters_per_second_squared"
    ] == pytest.approx(9.81)
    assert (
        standard.jump_height_takeoff_frame_meters.value
        != common.jump_height_takeoff_frame_meters.value
    )


def test_steps_no_registry_entry_describes_are_listed_rather_than_hidden(jump):
    unregistered = jump.unregistered_methods
    assert "takeoff_velocity.impulse_momentum" in unregistered
    assert "jump_height.from_takeoff_velocity" in unregistered


def test_flight_time_height_is_a_separate_construct_and_says_so():
    height = pf.jump_height_from_flight_time(0.5)
    assert height.provenance.method_id == "jump_height.from_flight_time"
    assert height.unit == "meters"
    assert height.provenance.bound_parameters["flight_time_seconds"] == pytest.approx(0.5)
    assert height.provenance.acquisition_complete is False
