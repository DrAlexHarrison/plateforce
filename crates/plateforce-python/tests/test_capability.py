"""What this surface says it can do, asked of the wheel that is running.

The manifest is read off the module's own exports rather than from a list beside them, so an
entry point that goes away shortens the array. A manifest generated inside the shared crate
would agree with itself whatever this wheel could actually do.
"""

import inspect
import json

import plateforce as pf

# Every member the acquisition block holds, written out rather than read from
# `pf.Acquisition`. Both sides of a comparison against the same source agree while five
# members become four, and a manifest naming four sends a reader to find four.
MEMBERS_A_READER_IS_TOLD_TO_FIND = [
    "filter_at_capture",
    "tare_state",
    "plate_natural_frequency_hz",
    "floor_surface",
    "firmware_version",
]

# The floor CAPABILITY.json declares, restated here so a change to it fails in this file too
# rather than only in the cross-surface gate, which needs three other surfaces built to run.
REQUIRED_OPERATIONS = [
    "analyse",
    "capability",
    "parse_force_file",
    "registry_census",
    "registry_show",
    "spread",
    "version",
]


def manifest():
    envelope = json.loads(pf.capability_json())
    assert "ok" in envelope, envelope
    return envelope["ok"]


def test_every_name_this_wheel_offers_says_what_it_can_do():
    """A class or a function added without a decision about the manifest would otherwise
    report nothing and the cross-surface gate would pass while the surfaces diverged."""
    unruled = pf._entry_points_with_no_operations_ruled()
    assert unruled == [], f"no operations ruled for {unruled}"


def test_the_wheel_reaches_every_operation_the_manifest_requires():
    reached = manifest()["operations"]
    assert [operation for operation in REQUIRED_OPERATIONS if operation not in reached] == []


def test_the_manifest_carries_the_shared_tables_every_surface_links():
    published = manifest()
    assert published["schema"] == "plateforce.capability/1"
    assert published["plateforce_version"] == pf.__version__
    assert len(published["methods"]) > 1, "a manifest naming one rule has said nothing"
    assert len(published["refusal_codes"]) > 1
    # Every code a shell reads has a status beside it, so a caller branching on the exit
    # status and one branching on the code are reading one decision.
    assert all(
        isinstance(code["exit_code"], int) and code["code"] for code in published["refusal_codes"]
    )


def test_the_operations_move_when_an_entry_point_does():
    """The array is derived rather than declared, so a name that is not an entry point
    contributes nothing and a name that is contributes what it was ruled to."""
    assert "spread" in manifest()["operations"]
    assert hasattr(pf, "spread")


def test_the_containers_reported_are_the_ones_this_build_writes():
    written = manifest()["output_formats"]
    assert "csv" in written, "a batch result writes csv"
    assert ("parquet" in written) == hasattr(pf.BatchResult, "write_parquet")


def test_the_manifest_is_one_document_however_often_it_is_asked():
    assert pf.capability_json() == pf.capability_json()


def test_the_manifest_names_every_member_of_the_acquisition_block():
    """A listing naming four of five members sends a reader to find four, and a block filled
    to four fingerprints as incomplete while reading like a complete one."""
    published = manifest()["acquisition"]["members"]

    unpublished = [m for m in MEMBERS_A_READER_IS_TOLD_TO_FIND if m not in published]
    assert unpublished == [], (
        f"{len(unpublished)} of {len(MEMBERS_A_READER_IS_TOLD_TO_FIND)} members are named "
        f"nowhere in the manifest: {unpublished}"
    )

    # The other direction, against the class this wheel hands a caller rather than against
    # the list above, so a member the block gains and the manifest drops is a failure rather
    # than a name nobody looked for.
    held = list(inspect.signature(pf.Acquisition).parameters)
    assert held, "the signature read nothing, so its verdict is about inspect and not the block"
    assert [m for m in held if m not in published] == []
    assert [m for m in published if m not in held] == []


def test_the_block_this_wheel_declares_is_the_block_its_constructor_takes():
    """Two directions. A wheel that takes a block and says it does not sends a reader to
    another surface for the recording; one that says it does and cannot is the claim the
    cross-surface comparison exists to make visible."""
    taken = "acquisition" in inspect.signature(pf.Trial).parameters
    declared = manifest()["acquisition"]["stated_by_caller"]
    assert declared == taken, (
        f"the manifest says the block is {'stated here' if declared else 'absent here'}, "
        f"and Trial {'takes' if taken else 'does not take'} one"
    )
