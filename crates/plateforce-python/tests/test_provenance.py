"""What survives the boundary with a number.

A result crossing into Python keeps the method that produced it. These tests are the ones
that fail if it ever becomes a bare float.
"""

import json
import re

import pytest

import plateforce as pf

from conftest import DECLARED_REVISION, SAMPLE_RATE_HZ

# Where the engine puts the figure. An account opens with the value and its unit, so whether
# one claims a measurement is readable on the first line and nowhere else.
OPENS_WITH_A_FIGURE = re.compile(r"^\s*-?\d")


@pytest.fixture
def jump(trial, bound_methods):
    epoch, onset, takeoff = bound_methods
    return pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)


# The engine's name for the quantity each getter answers for. Two of them are spelled
# differently here from the way the engine spells them, so the pairing has to be written; the
# guard below asserts it covers every getter that hands back a number, so one added without an
# entry reddens rather than going unread.
GETTER_QUANTITIES = {
    "flight_time_seconds": "flight_time_seconds",
    "jump_height_flight_time_meters": "jump_height_from_flight_time_meters",
    "jump_height_takeoff_frame_meters": "jump_height_from_takeoff_meters",
    "net_impulse_newton_seconds": "net_impulse_newton_seconds",
    "onset_time_seconds": "onset_time_seconds",
    "reactive_strength_index_modified": "reactive_strength_index_modified",
    "system_mass_kilograms": "system_mass_kilograms",
    "system_weight_newtons": "system_weight_newtons",
    "takeoff_time_seconds": "takeoff_time_seconds",
    "takeoff_velocity_meters_per_second": "takeoff_velocity_meters_per_second",
    "time_to_takeoff_seconds": "time_to_takeoff_seconds",
}


def as_data(provenance):
    """One record and every record above it, as plain data two of them can be compared on."""
    return (
        provenance.method_id,
        provenance.method_source,
        provenance.preset,
        dict(provenance.bound_parameters),
        dict(provenance.enumerated_choices),
        provenance.registry_version,
        provenance.registry_declared_version,
        provenance.registry_digest,
        provenance.acquisition_complete,
        provenance.manual_override,
        provenance.placed_by_hand_at_sample,
        tuple(as_data(step) for step in provenance.depends_on),
    )


def getters_handing_back_a_number(jump):
    return {
        name
        for name in dir(jump)
        if not name.startswith("_") and isinstance(getattr(jump, name), pf.Measured)
    }


def test_a_number_reached_two_ways_carries_one_record(landing_trial, bound_methods):
    """`value()` and the getter beside it are two routes to one number, and a reader who took
    the first used to be handed a different record from a reader who took the second: a step
    naming the arithmetic with no values and no inputs, against a chain naming the whole
    pipeline. Which route somebody happened to take is not a fact about the analysis.

    The trace lands, so every getter answers and nothing below passes by being skipped."""
    epoch, onset, takeoff = bound_methods
    jump = pf.analyse_countermovement_jump(landing_trial, epoch, onset, takeoff)

    held = getters_handing_back_a_number(jump)
    assert held == set(GETTER_QUANTITIES), (
        "a getter hands back a number and this guard does not know which quantity it is: "
        f"{sorted(held.symmetric_difference(GETTER_QUANTITIES))}"
    )

    for getter, quantity in sorted(GETTER_QUANTITIES.items()):
        by_name = getattr(jump, getter)
        by_key = jump.value(quantity)
        assert by_name.value == by_key.value, getter
        assert by_name.unit == by_key.unit, getter
        assert as_data(by_name.provenance) == as_data(by_key.provenance), getter
        assert by_name.describe() == by_key.describe(), getter


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
        "jump_height_takeoff_frame_meters": ("meters", "jumpheight.takeoff.impulse_momentum"),
    }
    for attribute, (unit, method_id) in expected.items():
        result = getattr(jump, attribute)
        assert result.unit == unit, attribute
        assert result.provenance.method_id == method_id, attribute


def test_manual_landmarks_carry_the_exact_samples_the_reader_placed(trial, bound_methods):
    """The 2 of 2 manual landmarks keep their flags and exact samples."""
    epoch, onset, takeoff = bound_methods
    placed = pf.analyse_countermovement_jump(
        trial,
        epoch,
        onset,
        takeoff,
        onset_index=1300,
        takeoff_index=1800,
    )
    placed_records = [
        placed.onset_time_seconds.provenance,
        placed.takeoff_time_seconds.provenance,
    ]
    assert [record.manual_override for record in placed_records] == [True, True]
    assert [record.placed_by_hand_at_sample for record in placed_records] == [1300, 1800]


def test_automatically_placed_landmarks_carry_no_manual_marker(trial, bound_methods):
    """The 2 of 2 automatic controls carry neither a manual flag nor a manual sample."""
    epoch, onset, takeoff = bound_methods
    automatic = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)
    automatic_records = [
        automatic.onset_time_seconds.provenance,
        automatic.takeoff_time_seconds.provenance,
    ]
    assert [record.manual_override for record in automatic_records] == [False, False]
    assert [record.placed_by_hand_at_sample for record in automatic_records] == [None, None]


def test_the_bound_parameters_travel_with_the_number(jump):
    """Every value the rule read, not only the ones the caller stated. A value a rule
    chose for itself moved the number exactly as much as one that was asked for."""
    onset = jump.onset_time_seconds.provenance
    assert onset.bound_parameters["k"] == 5.0
    # The backtrack is a registry entry of its own. Recording its offset against the
    # threshold rule would put the parameter on a row that does not carry it, so a reader
    # looking the id up would not find the value that moved the number.
    assert "offset_ms" not in onset.bound_parameters
    backtrack = onset.parameters_of("onset.op.backward_offset_fixed")
    assert backtrack["offset_ms"] == 30.0
    assert "offset_ms" in jump.assumed_parameters
    epoch = jump.system_weight_newtons.provenance
    assert epoch.bound_parameters["duration"] == 1.0


def test_the_registry_version_travels_with_the_number(jump):
    assert jump.jump_height_takeoff_frame_meters.provenance.registry_version == "fixture-1"


def test_the_pin_and_the_registrys_own_claim_are_two_fields(jump, registry):
    """A revision the caller cited and one the data claims are different facts.

    The terminal and the browser published the second under the first's name until
    2026-08-03, so every unpinned run told a reader the operator had chosen a revision no
    operator had chosen. Both are read off one result here and asserted against each other:
    a fixture whose claim equalled the pin would pass whichever field the value came from.
    """
    provenance = jump.jump_height_takeoff_frame_meters.provenance

    assert provenance.registry_version == "fixture-1"
    assert provenance.registry_declared_version == DECLARED_REVISION
    assert provenance.registry_version != provenance.registry_declared_version

    # And the same two questions asked of the registry the notebook is holding.
    assert registry.version == "fixture-1"
    assert registry.declared_version == DECLARED_REVISION

    # Every step of the chain, not the reported one alone. A record that carried the claim
    # only where it was asserted would pass an assertion made in that one place.
    for step in provenance.flattened():
        assert step.registry_declared_version == DECLARED_REVISION, step.method_id


def test_the_declared_revision_is_not_recoverable_from_the_digest(registry, registry_path):
    """Which is why a result carries it rather than a reader deriving it.

    The walk that measures the digest reads the toml files alone, so rewriting the VERSION
    file leaves the digest where it was. A reader holding only a digest cannot say which
    revision the registry called itself.
    """
    (registry_path / "VERSION").write_text("fixture-declares-something-else\n")
    try:
        renamed = pf.Registry.load(registry_path)
        assert renamed.digest == registry.digest, "the digest moved, and it should not have"
        assert renamed.declared_version == "fixture-declares-something-else"
        assert renamed.declared_version != registry.declared_version
    finally:
        (registry_path / "VERSION").write_text(DECLARED_REVISION + "\n")


def test_the_registry_digest_travels_with_the_number(jump, registry):
    provenance = jump.jump_height_takeoff_frame_meters.provenance
    assert provenance.registry_digest == registry.digest
    assert provenance.registry_digest.startswith("content-")


def test_an_unpinned_registry_leaves_the_version_unset_and_still_names_the_files(
    trial, registry_path
):
    unpinned = pf.Registry.load(registry_path)
    height = pf.analyse_countermovement_jump(
        trial,
        unpinned.method("bwepoch.fixed_window").bind(duration=1.0),
        unpinned.method("onset.threshold.noise_relative").bind(k=5.0),
        unpinned.method("takeoff.threshold.absolute_force").bind(
            threshold_n=20.0, persistence_ms=30.0
        ),
    ).jump_height_takeoff_frame_meters

    assert height.provenance.registry_version is None
    assert height.provenance.registry_digest == unpinned.digest
    # Unpinned, and the registry's own claim still travels, under its own name and worded
    # as the registry's rather than as the caller's.
    assert height.provenance.registry_declared_version == DECLARED_REVISION
    assert (
        f"registry declaring {DECLARED_REVISION} ({unpinned.digest})" in height.describe()
    )
    assert "pinned to" not in height.describe()


def test_a_height_computed_without_reading_a_registry_names_no_files():
    height = pf.jump_height_from_flight_time(0.5)
    assert height.provenance.registry_digest is None
    assert height.provenance.registry_version is None

    pinned = pf.jump_height_from_flight_time(0.5, registry_version="my-lab-2026-03")
    assert pinned.provenance.registry_version == "my-lab-2026-03"
    assert pinned.provenance.registry_digest is None


def test_choices_that_are_not_numbers_travel_too(jump):
    onset = jump.onset_time_seconds.provenance
    assert onset.enumerated_choices["sd_convention"] == "sample"
    composed = {step.method_id: step.enumerated_choices for step in onset.flattened()}
    assert composed["onset.op.direction"]["direction"] == "below_only"
    assert composed["onset.op.crossing_selection"]["selection"] == "first"


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


def test_describe_shows_the_value_and_the_whole_chain(jump, registry):
    described = jump.jump_height_takeoff_frame_meters.describe()
    assert "meters" in described
    assert "jumpheight.takeoff.impulse_momentum" in described
    assert "onset.threshold.noise_relative" in described
    assert "bwepoch.fixed_window" in described
    # Each revision worded as whose it is, and both on the line. A sentence that printed one
    # of them bare would read the same whether the caller cited it or the registry claimed
    # it about itself, which is the sentence this line used to print.
    assert (
        f"registry pinned to fixture-1 declaring {DECLARED_REVISION} ({registry.digest})"
        in described
    )


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
    """Every step this analysis performs now resolves to a registry entry, so the list is
    empty. A step added without one appears here rather than travelling unnamed."""
    assert jump.unregistered_methods == []


def test_the_document_carries_an_account_of_every_number_it_reports(trial, bound_methods):
    """A notebook reads one account per value in the process, and the document it hands on
    carried none, so a number pasted out of a notebook left its account behind.

    Each account is read against the record beside it rather than against a sentence written
    here: the value it opens with, the unit that value carries, and the rule the record says
    produced it. A block filled with anything at all passes none of those."""
    epoch, onset, takeoff = bound_methods
    document = json.loads(
        pf._analyse_json(trial, weighing_epoch=epoch, onset=onset, takeoff=takeoff)
    )["ok"]

    valued = [metric for metric in document["metrics"] if metric["value"] is not None]
    # The denominator the sentence below is over. A run reporting almost nothing would
    # satisfy the comparison having looked at almost nothing.
    assert len(valued) >= 8, f"only {len(valued)} of {len(document['metrics'])} carried a value"

    silent = [
        metric["key"] for metric in valued if metric["key"] not in document["descriptions"]
    ]
    assert silent == [], (
        f"{len(silent)} of {len(valued)} quantities carrying a value gave no account: {silent}"
    )

    for metric in valued:
        account = document["descriptions"][metric["key"]]
        opening = account.splitlines()[0]
        assert opening == f"{metric['value']} {metric['unit']}", metric["key"]
        named = metric["computed_by"] or metric["contributing_method_ids"][0]
        assert named in account, f"the account of {metric['key']} never names {named}"


def test_a_number_and_its_account_report_one_value(trial, bound_methods):
    """The value a caller reads off the object is the value its account opens with.

    Two renderings of one number that round differently, or that came from two runs, are the
    defect this whole field exists against, and a document filled from a second analysis
    would pass every presence check above."""
    epoch, onset, takeoff = bound_methods
    document = json.loads(
        pf._analyse_json(trial, weighing_epoch=epoch, onset=onset, takeoff=takeoff)
    )["ok"]
    shaped = pf.analyse_countermovement_jump(trial, epoch, onset, takeoff)

    compared = 0
    for getter, quantity in GETTER_QUANTITIES.items():
        measured = getattr(shaped, getter)
        # A getter answers None where no rule produced the quantity at all, which is the
        # state the case below is over.
        if measured is None or measured.value is None:
            continue
        account = document["descriptions"][quantity]
        assert account.splitlines()[0] == f"{measured.value} {measured.unit}"
        compared += 1
    assert compared >= 8, f"only {compared} of {len(GETTER_QUANTITIES)} getters were compared"


def test_a_quantity_with_no_value_gives_no_account_claiming_a_measurement(
    trial, bound_methods
):
    """The control on the case above. A block that simply held every key would pass it, and a
    sentence asserting a figure nobody computed is what this field exists against.

    What an absent quantity may not carry is an account opening with a value and a unit, which
    is where the case above reads the figure. It may carry its rule's own sentence, and on
    these quantities it has the most to say: a landing rule declining a flight time where the
    plate never unloads is an answer, and forbidding the account would hide the declining rule
    on exactly the quantities a reader needs it on. Same property as the browser's, in
    `scripts/check-account.mjs`, read the same way.

    The shared trace ends in flight, so nothing places a touchdown and the quantities that
    rest on one report no number."""
    epoch, onset, takeoff = bound_methods
    document = json.loads(
        pf._analyse_json(trial, weighing_epoch=epoch, onset=onset, takeoff=takeoff)
    )["ok"]

    absent = [metric["key"] for metric in document["metrics"] if metric["value"] is None]
    assert absent != [], "every quantity carried a value on a trial written to leave some without one"
    accounted = [key for key in absent if key in document["descriptions"]]
    # Without this the sweep below runs over nothing and reports clean on a build that dropped
    # every declining rule's sentence, which is the state this quantity's reader is worst served by.
    assert accounted == absent, (
        f"{len(absent) - len(accounted)} of {len(absent)} quantities with no value name no rule: "
        f"{sorted(set(absent) - set(accounted))}"
    )
    def opening(key):
        return document["descriptions"][key].splitlines()[0]

    # The same reading over the quantities that did produce a number, where every one has to
    # match. A predicate that had stopped recognising a figure would report the line below
    # clean over any account at all, and this is the population that cannot let it.
    valued = [
        metric["key"]
        for metric in document["metrics"]
        if metric["value"] is not None and metric["key"] in document["descriptions"]
    ]
    blind = [key for key in valued if not OPENS_WITH_A_FIGURE.match(opening(key))]
    assert valued != [] and blind == [], (
        f"{len(blind)} of {len(valued)} quantities carrying a value do not open on a figure, so "
        f"the reading below cannot see one: {[(key, opening(key)) for key in blind[:3]]}"
    )

    claiming = [key for key in accounted if OPENS_WITH_A_FIGURE.match(opening(key))]
    assert claiming == [], (
        f"{len(claiming)} of {len(absent)} quantities with no value open on a figure: "
        + "; ".join(f'{key} opens "{opening(key)}"' for key in claiming)
    )


def test_flight_time_height_is_a_separate_construct_and_says_so():
    height = pf.jump_height_from_flight_time(0.5)
    assert height.provenance.method_id == "jumpheight.takeoff.flight_time"
    assert height.unit == "meters"
    assert height.provenance.bound_parameters["flight_time_seconds"] == pytest.approx(0.5)
    assert height.provenance.acquisition_complete is False
