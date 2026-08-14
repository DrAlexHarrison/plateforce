"""The gravity beside a number is the one that produced it.

This surface hand-listed four numbers and attached the request's gravity to each, which named
a value that had not produced the number wherever a rule ran at one of its own. Modified
reactive strength moves with gravity and was not on the list at all.

Every set here is computed by moving the gravity and reading which numbers followed, and the
keys come from the analysis rather than from a list written here. Four attribute names written
down would go stale the day a fifth started reading gravity, and would pass while doing it.
"""

import pytest

import plateforce as pf

GRAVITY = "gravity_meters_per_second_squared"

# The two constants the tools argue over. Gravity varies by half a percent across the Earth's
# surface, fifteen times this gap, so a guard that holds here holds on anything a plate owner
# would state.
STANDARD = 9.80665
PUBLISHED = 9.81


def keyword(gravity):
    return {} if gravity is None else {GRAVITY: gravity}


def analysed(trial, bound_methods, gravity=None):
    epoch, onset, takeoff = bound_methods
    return pf.analyse_countermovement_jump(
        trial, epoch, onset, takeoff, **keyword(gravity)
    )


def numbers(trial, bound_methods, gravity=None):
    """Every number this surface hands a caller, read off the class rather than from a list
    written here, so a quantity that gains a getter is covered without an edit.

    A `Measured` is the object a caller keeps: `height = jump.jump_height_takeoff_frame_meters`
    travels on its own, away from the result it came out of, which is why the record it carries
    has to be complete by itself.
    """
    jump = analysed(trial, bound_methods, gravity)
    held = {}
    for name in dir(jump):
        if name.startswith("_"):
            continue
        got = getattr(jump, name)
        if isinstance(got, pf.Measured):
            held[name] = got
    assert held, "the result handed back no numbers at all"
    return held


def gravity_recorded_for(measured):
    return dict(measured.provenance.bound_parameters).get(GRAVITY)


def moved_between(trial, bound_methods, one, other):
    before = numbers(trial, bound_methods, one)
    after = numbers(trial, bound_methods, other)
    assert before.keys() == after.keys(), "the two analyses handed back different numbers"
    return {name for name, held in before.items() if after[name].value != held.value}


def test_every_number_a_moving_gravity_moves_carries_the_gravity_that_moved_it(
    landing_trial, bound_methods
):
    """The set is measured, and the guard first requires it to be non-empty: a build where
    gravity moved nothing would otherwise satisfy every line below while proving none."""
    moved = moved_between(landing_trial, bound_methods, STANDARD, PUBLISHED)
    assert moved, f"no number moved between {STANDARD} and {PUBLISHED}"

    for requested in (STANDARD, PUBLISHED):
        held = numbers(landing_trial, bound_methods, requested)
        for name in sorted(moved):
            recorded = gravity_recorded_for(held[name])
            assert recorded is not None, f"{name} moved with gravity and names none"
            assert recorded == pytest.approx(
                requested
            ), f"{name} ran at {requested} and its record names {recorded}"


def test_a_number_a_gravity_never_reached_does_not_claim_one(
    landing_trial, bound_methods
):
    """The other half, and the half a guard reaching only for presence cannot see. Net impulse
    is integrated over an interval and divided by nothing, so a gravity beside it would be a
    dependence the number does not have."""
    held = numbers(landing_trial, bound_methods, STANDARD)
    held_still = set(held) - moved_between(landing_trial, bound_methods, STANDARD, PUBLISHED)
    assert held_still, "every number moved, so nothing here is being tested"

    for name in sorted(held_still):
        assert (
            gravity_recorded_for(held[name]) is None
        ), f"{name} is the same at {STANDARD} and {PUBLISHED} and names a gravity anyway"


def test_the_flight_time_height_reports_the_gravity_it_ran_at(
    landing_trial, bound_methods
):
    """The defect this file was written for. `jumpheight.takeoff.flight_time` declares gravity
    required and answers it with no default, so the height runs at the gravity the analysis is
    bound to and the record has to name that one and not another.

    The check is the closed form rather than a second value read from the same place, so a
    record quoting a gravity the height was not computed at cannot agree with the number beside
    it. Run at both constants, because one alone holds on a build where this height reads no
    gravity at all: the two heights are required to differ, which is the same claim as the
    record's, made on the number instead of on the record."""
    heights = {}
    for requested in (STANDARD, PUBLISHED):
        jump = analysed(landing_trial, bound_methods, requested)
        height = jump.jump_height_flight_time_meters
        assert height is not None, "the trace lands, so the flight-time height has to exist"

        recorded = gravity_recorded_for(height)
        flight = jump.flight_time_seconds.value
        assert recorded == pytest.approx(
            requested
        ), f"the height ran at {requested} and its record names {recorded}"
        assert height.value == pytest.approx(recorded * flight * flight / 8.0)
        heights[requested] = height.value

    assert heights[STANDARD] != heights[PUBLISHED], (
        "the height is the same at both constants, so nothing above is about gravity"
    )


def test_a_gravity_nobody_was_asked_about_is_listed_among_the_values_nobody_chose(
    landing_trial, bound_methods
):
    """One value, two claims, and the claim is what the caller is being told apart by."""
    quiet = analysed(landing_trial, bound_methods)
    stated = analysed(landing_trial, bound_methods, STANDARD)

    assert GRAVITY in quiet.assumed_parameters
    assert GRAVITY not in stated.assumed_parameters


def test_the_sweep_moves_the_panel_when_gravity_is_the_axis(
    landing_trial, bound_methods
):
    """A rule answering with its entry's own constant regardless would make the panel print a
    spread of zero over a knob that had moved. The panel is where a reader is told how far a
    choice carries the number, so a zero here is worse than no panel."""
    epoch, onset, takeoff = bound_methods
    spread = pf.spread(
        landing_trial,
        "jump_height_from_flight_time_meters",
        weighing_epoch=epoch,
        onset=onset,
        takeoff=takeoff,
        vary={f"global.{GRAVITY}": [9.79, STANDARD, PUBLISHED]},
    )
    assert spread.succeeded == 3
    assert spread.spread_absolute > 0.0
