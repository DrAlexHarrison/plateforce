"""Saved plates from a notebook.

The store is the terminal's. A plate written by `plateforce plate save` is the plate
`Plate.saved` reaches, so the assertions that matter here are about one answer rather than
about two that happen to agree: the revision a stated plate hashes to is the revision a saved
one hashes to, and both are the string a result attributes to the plate it ran under.
"""

import json

import pytest

import plateforce as pf

SAMPLE_RATE_HZ = 1200.0

MEMBERS = {
    "filter_at_capture": "none",
    "tare_state": "tared_before_trial",
    "plate_natural_frequency_hz": 400.0,
    "floor_surface": "concrete",
    "firmware_version": "2.1",
}


@pytest.fixture
def folder(tmp_path):
    """A plates folder this test owns, so nothing here writes into the machine's own."""
    return str(tmp_path / "plates")


@pytest.fixture
def block():
    return pf.Acquisition(**MEMBERS)


def test_a_saved_plate_fills_a_complete_block_on_a_later_run(folder, block):
    pf.save_plate("lab-kistler-1", block, plates_directory=folder)
    trial = pf.Trial(
        [700.0] * 1200,
        sample_rate_hz=SAMPLE_RATE_HZ,
        plate=pf.Plate.saved("lab-kistler-1", folder),
    )

    assert trial.acquisition_complete
    assert trial.acquisition.missing == []
    assert trial.plate.name == "lab-kistler-1"
    assert trial.plate.superseded_members == {}


def test_a_member_stated_beside_a_plate_wins_and_the_record_says_what_it_replaced(
    folder, block
):
    pf.save_plate("lab-kistler-1", block, plates_directory=folder)
    trial = pf.Trial(
        [700.0] * 1200,
        sample_rate_hz=SAMPLE_RATE_HZ,
        acquisition=pf.Acquisition(firmware_version="2.2"),
        plate=pf.Plate.saved("lab-kistler-1", folder),
    )

    assert trial.acquisition.firmware_version == "2.2"
    assert trial.plate.superseded_members == {"firmware_version": "2.1"}


def test_a_stated_plate_and_a_saved_one_carry_one_revision(folder, block):
    saved = pf.save_plate("lab-kistler-1", block, plates_directory=folder)
    stated = pf.Plate("lab-kistler-1", block)

    assert stated.revision == saved.plate.revision
    assert stated.path is None
    assert saved.plate.path is not None


def test_saving_over_a_name_hands_back_what_it_replaced(folder, block):
    first = pf.save_plate("lab-kistler-1", block, plates_directory=folder)
    edited = pf.Acquisition(**{**MEMBERS, "firmware_version": "2.2"})
    second = pf.save_plate("lab-kistler-1", edited, plates_directory=folder)

    assert second.plate.revision != first.plate.revision
    assert second.replaced.revision == first.plate.revision
    assert second.replaced_members == {"firmware_version": ("2.1", "2.2")}


def test_a_plate_short_of_a_member_says_which_and_the_run_reports_it(folder):
    partial = pf.Acquisition(tare_state="tared_before_trial")
    saved = pf.save_plate("lab-partial", partial, plates_directory=folder)

    assert not saved.plate.is_complete
    assert "firmware_version" in saved.plate.missing
    trial = pf.Trial(
        [700.0] * 1200,
        sample_rate_hz=SAMPLE_RATE_HZ,
        plate=pf.Plate.saved("lab-partial", folder),
    )
    assert not trial.acquisition_complete


def test_the_plates_this_machine_holds_are_named_in_order(folder, block):
    for name in ("lab-b", "lab-a"):
        pf.save_plate(name, block, plates_directory=folder)
    assert [plate.name for plate in pf.saved_plates(folder)] == ["lab-a", "lab-b"]
    assert pf.plates_folder(folder) == folder


def test_a_forgotten_plate_is_gone_and_the_ones_beside_it_are_not(folder, block):
    pf.save_plate("lab-a", block, plates_directory=folder)
    pf.save_plate("lab-b", block, plates_directory=folder)

    pf.forget_plate("lab-a", folder)

    assert [plate.name for plate in pf.saved_plates(folder)] == ["lab-b"]
    with pytest.raises(pf.PlateforceError):
        pf.Plate.saved("lab-a", folder)


def test_a_plate_nobody_saved_is_refused_under_a_code_rather_than_a_sentence(folder):
    with pytest.raises(pf.PlateforceError) as refused:
        pf.Plate.saved("lab-kistler-9", folder)
    assert refused.value.code == "file_not_read"
    assert "lab-kistler-9" in str(refused.value)


@pytest.mark.parametrize("name", ["../secrets", "lab/1", "", "lab.1"])
def test_a_name_that_would_reach_another_folder_is_refused(name, block, folder):
    with pytest.raises(pf.PlateforceError):
        pf.Plate(name, block)
    with pytest.raises(pf.PlateforceError):
        pf.save_plate(name, block, plates_directory=folder)


def test_a_run_that_named_no_plate_has_nothing_to_attribute(block):
    trial = pf.Trial([700.0] * 1200, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=block)
    assert trial.acquisition_complete
    assert trial.plate is None


def test_the_record_carries_the_plate_a_run_was_filled_from(folder, block, bound_methods):
    """The whole point of the feature reaching this surface.

    A notebook that could name a plate and then published a document saying nothing about it
    would leave a reader holding the members with no way to see which plate they were typed
    into, which is the state this surface was in.
    """
    weighing, onset, takeoff = bound_methods
    pf.save_plate("lab-kistler-1", block, plates_directory=folder)
    trial = pf.Trial(
        [700.0] * 1200,
        sample_rate_hz=SAMPLE_RATE_HZ,
        acquisition=pf.Acquisition(firmware_version="2.2"),
        plate=pf.Plate.saved("lab-kistler-1", folder),
    )

    # The envelope every surface answers in, so a refusal and a result are one shape.
    document = json.loads(
        pf._analyse_json(trial, weighing_epoch=weighing, onset=onset, takeoff=takeoff)
    )["ok"]

    assert document["acquisition_complete"] is True
    assert document["acquisition"]["firmware_version"] == "2.2"
    assert document["plate_profile"]["name"] == "lab-kistler-1"
    assert document["plate_profile"]["superseded_members"] == {"firmware_version": "2.1"}


def test_a_run_with_no_plate_leaves_the_field_off_the_wire_rather_than_null(
    block, bound_methods
):
    """Absent rather than null, the way every surface writes it: a run with no saved plate
    behind it has nothing to attribute, and null would read as an attribution to nothing."""
    weighing, onset, takeoff = bound_methods
    trial = pf.Trial([700.0] * 1200, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=block)

    # The envelope every surface answers in, so a refusal and a result are one shape.
    document = json.loads(
        pf._analyse_json(trial, weighing_epoch=weighing, onset=onset, takeoff=takeoff)
    )["ok"]

    assert "plate_profile" not in document
    assert document["acquisition"]["firmware_version"] == "2.1"
