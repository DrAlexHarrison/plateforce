"""A notebook states the athlete's mass and reads it back as its own claim.

The athlete's mass is not the weighed system mass: system weight includes any bar and
bodyweight does not. A surface with no way to say which leaves a caller substituting the
number beside it, and nothing in the record can tell the two apart afterwards.

The absence half is the load-bearing one. A row written whatever the caller said would
report the software's silence as their choice.
"""

import math

import pytest

import plateforce as pf

BODY_MASS = "body_mass_kilograms"
GRAVITY = "gravity_meters_per_second_squared"

STATED_MASS_KILOGRAMS = 61.5
STANDARD_GRAVITY = 9.80665


def analysed(trial, bound_methods, **stated):
    epoch, onset, takeoff = bound_methods
    return pf.analyse_countermovement_jump(trial, epoch, onset, takeoff, **stated)


def by_name(jump):
    """Every value the analysis was bound to, against the name the record reports it by."""
    return {bound.name: bound for bound in jump.bound_globals}


def test_a_stated_mass_is_on_the_record_as_the_callers_own_claim(
    landing_trial, bound_methods
):
    bound = by_name(
        analysed(landing_trial, bound_methods, body_mass_kilograms=STATED_MASS_KILOGRAMS)
    )
    assert bound[BODY_MASS].value == STATED_MASS_KILOGRAMS
    assert bound[BODY_MASS].source == "stated"
    # The unit travels with the number, because a mass read back without one is a number a
    # caller has to assume the units of.
    assert bound[BODY_MASS].unit == "kilograms"
    assert bound[BODY_MASS].unit_symbol == "kg"


def test_a_run_that_states_no_mass_carries_no_row_for_one(landing_trial, bound_methods):
    bound = by_name(analysed(landing_trial, bound_methods))
    assert BODY_MASS not in bound, bound
    # The population is more than one on purpose: a record holding no row at all would
    # satisfy the line above while proving nothing about the shape that holds a row.
    assert bound[GRAVITY].value == pytest.approx(STANDARD_GRAVITY)
    assert bound[GRAVITY].source == "assumed"


def test_the_record_reports_both_bound_values_when_both_are_stated(
    landing_trial, bound_methods
):
    bound = by_name(
        analysed(
            landing_trial,
            bound_methods,
            body_mass_kilograms=STATED_MASS_KILOGRAMS,
            gravity_meters_per_second_squared=9.81,
        )
    )
    assert {name: held.source for name, held in bound.items()} == {
        GRAVITY: "stated",
        BODY_MASS: "stated",
    }


@pytest.mark.parametrize(
    "kilograms",
    [0.0, -61.5, float("nan"), float("inf"), -math.inf],
)
def test_a_mass_that_is_not_a_positive_finite_number_is_refused_by_name(
    landing_trial, bound_methods, kilograms
):
    """Refused under the name the record reports the value by, rather than under the
    argument, so a caller comparing a refusal against a result reads one word."""
    with pytest.raises(pf.PlateforceError) as raised:
        analysed(landing_trial, bound_methods, body_mass_kilograms=kilograms)
    assert BODY_MASS in str(raised.value), str(raised.value)


def test_the_sweep_sends_the_mass_the_analysis_sends(landing_trial, bound_methods):
    """A sweep's unvaried combination has to be the request the caller's own analysis sends,
    or the spread is around a different result than the one they read."""
    epoch, onset, takeoff = bound_methods
    swept = pf.spread(
        landing_trial,
        quantity="system_weight_newtons",
        slot="weighing",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        body_mass_kilograms=STATED_MASS_KILOGRAMS,
    )
    assert swept.succeeded, "the sweep produced no combination, so it proves nothing"

    with pytest.raises(pf.PlateforceError):
        pf.spread(
            landing_trial,
            quantity="system_weight_newtons",
            slot="weighing",
            weighing_epoch=epoch,
            onset=onset,
            takeoff=takeoff,
            body_mass_kilograms=0.0,
        )
