"""A folder run from a notebook.

The batch entry point had no test at all, and it could not bind a rule for anything computed
from the landmarks, could not state a value for one, and returned neither the quality signals
nor the gate findings the other surfaces return. So a notebook reading a folder got the
narrowest answer of the four surfaces and nothing said so.

The registry here is the fixture one from `conftest`, so a failure means the binding changed
rather than that the shipped registry gained an entry.
"""

import inspect

import numpy as np
import pytest

from conftest import SAMPLE_RATE_HZ


@pytest.fixture
def trial_folder(tmp_path, force_newtons):
    """Four traces of one subject, named so a declared pattern yields a subject."""
    folder = tmp_path / "trials"
    folder.mkdir()
    for trial in range(1, 5):
        shifted = force_newtons + np.float64(trial)
        (folder / f"AT01_{trial}.force.txt").write_text(
            "\n".join(f"{value:.6f}" for value in shifted)
        )
    return folder


def run(folder, registry_path, **extra):
    import plateforce as pf

    delimiter = extra.pop("delimiter", "\t")
    force_column_index = extra.pop("force_column_index", 0)
    return pf.batch(
        folder,
        registry=registry_path,
        weighing="bwepoch.fixed_window",
        onset="onset.threshold.noise_relative",
        # k is published four ways, so an unnamed value is a choice the engine refuses to make.
        onset_parameters={"k": 5.0},
        takeoff="takeoff.threshold.absolute_force",
        sentinel=None,
        delimiter=delimiter,
        force_column_index=force_column_index,
        sample_rate_hz=SAMPLE_RATE_HZ,
        trial_file_suffixes=[".force.txt"],
        resolved=["system_weight", "movement_onset", "takeoff"],
        **extra,
    )


def test_a_folder_run_has_no_sample_rate_default():
    import plateforce as pf

    rate = inspect.signature(pf.batch).parameters["sample_rate_hz"]
    assert rate.default is inspect.Parameter.empty


def test_a_folder_run_refuses_a_sample_rate_nobody_stated(trial_folder, registry_path):
    """The single-file reader already requires the rate. A folder cannot quietly choose one
    because height and displacement move with the square of that choice."""
    import plateforce as pf

    with pytest.raises(TypeError) as refused:
        pf.batch(
            trial_folder,
            registry=registry_path,
            weighing="bwepoch.fixed_window",
            onset="onset.threshold.noise_relative",
            onset_parameters={"k": 5.0},
            takeoff="takeoff.threshold.absolute_force",
            sentinel=None,
            delimiter="\t",
            force_column_index=0,
            trial_file_suffixes=[".force.txt"],
            resolved=["system_weight", "movement_onset", "takeoff"],
        )
    assert "sample_rate_hz" in str(refused.value)


def test_a_folder_run_records_every_acquisition_member(
    trial_folder, registry_path, complete_acquisition
):
    complete = run(
        trial_folder,
        registry_path,
        acquisition=complete_acquisition,
    )
    record = complete.run.to_dict()
    assert sum(value is not None for value in record["acquisition"].values()) == 5, (
        "recorded 5 of 5 acquisition members"
    )
    assert record["acquisition_complete_count"] == complete.run.computed_count == 4, (
        "recorded complete acquisition for 4 of 4 computed trials"
    )


def test_only_a_complete_acquisition_block_publishes_a_fingerprint(
    trial_folder, registry_path, complete_acquisition
):
    """The complete, partial and unstated cases are controls for one another. A fingerprint
    is published only for 4 of 4 computed trials with all 5 of 5 acquisition members."""
    import plateforce as pf

    complete = run(
        trial_folder,
        registry_path,
        acquisition=complete_acquisition,
    )
    partial = run(
        trial_folder,
        registry_path,
        acquisition=pf.Acquisition(
            filter_at_capture="none",
            tare_state="tared_before_trial",
            plate_natural_frequency_hz=800.0,
            floor_surface="concrete",
        ),
    )
    unstated = run(trial_folder, registry_path)

    partial_record = partial.run.to_dict()
    unstated_record = unstated.run.to_dict()
    assert complete.run.run_fingerprint is not None
    assert complete.run.run_fingerprint.startswith("content-")

    assert sum(value is not None for value in partial_record["acquisition"].values()) == 4, (
        "recorded 4 of 5 stated acquisition members"
    )
    assert partial_record["acquisition_complete_count"] == 0
    assert partial.run.run_fingerprint is None

    assert sum(value is not None for value in unstated_record["acquisition"].values()) == 0, (
        "recorded 0 of 5 acquisition members when none were stated"
    )
    assert unstated_record["acquisition_complete_count"] == 0
    assert unstated.run.run_fingerprint is None


def test_a_changed_acquisition_block_changes_the_fingerprint(
    trial_folder, registry_path, complete_acquisition
):
    import plateforce as pf

    complete = run(
        trial_folder,
        registry_path,
        acquisition=complete_acquisition,
    )
    changed = run(
        trial_folder,
        registry_path,
        acquisition=pf.Acquisition(
            filter_at_capture="none",
            tare_state="tared_before_trial",
            plate_natural_frequency_hz=800.0,
            floor_surface="concrete",
            firmware_version="synthetic-1",
        ),
    )
    assert changed.run.run_fingerprint != complete.run.run_fingerprint


def test_to_pandas_converts_all_trial_rows(trial_folder, registry_path):
    pandas = pytest.importorskip("pandas")
    result = run(trial_folder, registry_path)

    frame = result.to_pandas()
    expected = [row["trial_id"] for row in result.results]
    assert isinstance(frame, pandas.DataFrame)
    assert len(frame.index) == len(expected) == 4, "converted 4 of 4 trial rows"
    assert frame["trial_id"].tolist() == expected


def test_to_arrow_converts_all_trial_rows(trial_folder, registry_path):
    pyarrow = pytest.importorskip("pyarrow")
    result = run(trial_folder, registry_path)

    table = result.to_arrow()
    expected = [row["trial_id"] for row in result.results]
    assert isinstance(table, pyarrow.Table)
    assert table.num_rows == len(expected) == 4, "converted 4 of 4 trial rows"
    assert table.column("trial_id").to_pylist() == expected


def test_a_separator_of_several_characters_is_refused_before_the_folder_is_read(
    trial_folder, registry_path
):
    import plateforce as pf

    with pytest.raises(pf.ParameterError) as raised:
        run(trial_folder, registry_path, delimiter="::")
    assert raised.value.code == "value_not_accepted"
    assert raised.value.parameter == "delimiter"
    assert raised.value.available == ["one character", "the word whitespace"]
    assert "::" in str(raised.value)


def test_a_folder_held_apart_by_runs_of_spaces_reads_by_naming_whitespace(
    tmp_path, force_newtons, registry_path
):
    folder = tmp_path / "space-separated"
    folder.mkdir()
    for trial in range(1, 5):
        (folder / f"AT01_{trial}.force.txt").write_text(
            "\n".join(
                f"{sample}     {value + trial:.6f}"
                for sample, value in enumerate(force_newtons)
            )
        )

    result = run(
        folder,
        registry_path,
        delimiter="whitespace",
        force_column_index=1,
    )
    assert result.run.computed_count == result.run.trial_count == 4
    assert '"delimiter":"whitespace"' in result.to_json()


def test_a_rule_computed_from_the_landmarks_reaches_the_table(trial_folder, registry_path):
    """The rule is bound, runs on every trial, and its quantity is a column of the result."""
    without = run(trial_folder, registry_path)
    assert "peak_force_newtons" not in without.quantities

    result = run(
        trial_folder,
        registry_path,
        derived={
            "analysis_window": "window_end.takeoff.detected",
            "peak_force": "force.peak.gross",
        },
    )
    assert "peak_force_newtons" in result.quantities
    assert result.units["peak_force_newtons"] == "newtons"
    answered = [
        row["values"]["peak_force_newtons"]
        for row in result.results
        if row["values"].get("peak_force_newtons") is not None
    ]
    assert len(answered) == result.run.trial_count == 4
    assert all(value > 0.0 for value in answered)


def test_a_value_stated_for_a_derived_rule_moves_the_number_and_reaches_the_record(
    trial_folder, registry_path
):
    """A wider centred average cannot report a higher peak, and the record names the width.

    The pair is what makes this a measurement rather than a smoke test: a run that ignored
    the value writes one number twice, and a run that recorded a constant writes one record
    twice.
    """
    seen = []
    for window_seconds in (0.0, 0.05):
        result = run(
            trial_folder,
            registry_path,
            derived={
                "analysis_window": "window_end.takeoff.detected",
                "peak_force": "force.peak.estimator",
            },
            derived_parameters={"peak_force": {"averaging_window_seconds": window_seconds}},
        )
        peak = result.results[0]["values"]["peak_force_newtons"]
        recorded = [
            row["value"]
            for row in result.provenance
            if row["method_id"] == "force.peak.estimator"
            and row["parameter"] == "averaging_window_seconds"
        ]
        assert recorded, f"the record names no averaging window at {window_seconds} s"
        assert {float(value) for value in recorded} == {window_seconds}
        seen.append(peak)

    assert seen[0] > seen[1], seen


def test_a_construct_this_build_runs_no_rule_for_is_refused_before_a_trial_is_read(
    trial_folder, registry_path
):
    with pytest.raises(ValueError) as raised:
        run(trial_folder, registry_path, derived={"not_a_construct": "anything"})
    assert "phase_model" in str(raised.value)


def test_an_id_filed_under_another_construct_is_refused_with_the_ones_filed_under_this_one(
    trial_folder, registry_path
):
    with pytest.raises(ValueError) as raised:
        run(
            trial_folder,
            registry_path,
            derived={"peak_force": "onset.threshold.absolute_force"},
        )
    said = str(raised.value)
    assert "force.peak." in said
    assert "onset.threshold.noise_relative" not in said


def test_the_relations_a_notebook_reads_are_the_ones_the_other_surfaces_write(
    trial_folder, registry_path
):
    """Every relation in the envelope is reachable as an attribute.

    Signals and exclusions reached CSV, the JSON envelope and the terminal and stopped at the
    Python boundary, so a notebook was the one surface that could not see what a run already
    knew about the numbers it was handing over.
    """
    import json

    result = run(trial_folder, registry_path)
    envelope = json.loads(result.to_json())["ok"]
    for relation in ("results", "provenance", "refusals", "warnings", "signals", "exclusions"):
        assert hasattr(result, relation), relation
        assert len(getattr(result, relation)) == len(envelope[relation]), relation


def test_the_request_digest_carries_which_registry_backed_the_rules(
    trial_folder, registry_path, tmp_path
):
    """The digest that identifies a run used to be blind to the registry it was run against.

    A folder run from a notebook passed an empty backed list, so the same rules over the same
    folder fingerprinted one way here and another from the terminal, which reads the ids off
    the registry it loaded. A digest that says two identical runs differ, or that two runs
    against different registries are one, is the thing the fingerprint exists not to do.
    """
    second = tmp_path / "registry-two"
    (second / "methods").mkdir(parents=True)
    (second / "constructs.toml").write_text((registry_path / "constructs.toml").read_text())
    (second / "methods" / "seed.toml").write_text(
        (registry_path / "methods" / "seed.toml").read_text()
        + """
[[method]]
id = "force.peak.net"
construct = "peak_force"
title = "The biggest force, system weight removed"
rule = "Peak force is the maximum of the force series less system weight."
status = "accepted"
confidence = "high"
"""
    )

    first_run = run(trial_folder, registry_path)
    second_run = run(trial_folder, second)
    assert first_run.run.request_digest != second_run.run.request_digest
    assert first_run.run.registry_digest != second_run.run.registry_digest


def _edge_sources(result):
    """Where the edge the conditioning rule read came from, on every trial in the folder."""
    return {
        row["source"]
        for row in result.provenance
        if row["method_id"] == "filter.none" and row["parameter"] == "passband_edge"
    }


def test_a_folder_run_states_what_conditioned_the_signal(trial_folder, registry_path):
    """The phase that produces the signal runs on every trial and a notebook folder run had no
    argument for it, so the record named the software on every row.

    The pair is what makes this a measurement: a run that ignored the argument reports the same
    source both times.
    """
    unstated = run(trial_folder, registry_path)
    assert _edge_sources(unstated) == {"assumed"}

    stated = run(
        trial_folder,
        registry_path,
        conditioning_options={"conditioned_force_signal": {"passband_edge": "none"}},
    )
    assert _edge_sources(stated) == {"stated"}


def test_a_folder_run_is_refused_an_edge_the_conditioning_rule_does_not_take(
    trial_folder, registry_path
):
    """Every trial declines, in the one sentence the other three surfaces decline in, and no
    trial reports a number measured on a signal the caller asked a rule not to produce."""
    result = run(
        trial_folder,
        registry_path,
        conditioning_options={"conditioned_force_signal": {"passband_edge": "20"}},
    )

    declined = [row for row in result.refusals if row["method_id"] == "filter.none"]
    assert len(declined) == result.run.trial_count == 4
    for row in declined:
        assert row["code"] == "value_not_accepted"
        assert row["parameter"] == "passband_edge"
        assert row["available"] == "none"

    answered = [
        row
        for row in result.results
        if row["values"].get("jump_height_from_takeoff_meters") is not None
    ]
    assert answered == []


def test_a_folder_run_reduces_an_athletes_trials_under_a_named_published_rule(
    trial_folder, registry_path
):
    """This surface exposed `aggregates` before it could produce one, so every call it could
    make returned an empty list, and a caller cannot tell that from a run with nothing to
    reduce. The control is the first assertion: the same folder without a rule still returns
    empty, so the second is about the reduction rather than about the fixture."""
    unreduced = run(trial_folder, registry_path)
    assert unreduced.aggregates == []

    reduced = run(
        trial_folder,
        registry_path,
        pattern="AT{subject}_{trial}",
        aggregate="mean_of_best_two",
        aggregate_n=2,
        aggregate_ranked_by="reactive_strength_index",
        aggregate_quantity=["jump_height_from_takeoff_meters"],
    )
    assert reduced.aggregates, "the run bound a published rule and reduced nothing"

    # The bound rule travels with the value. A reduction recording no method would be a mean
    # wearing a citation it never earned, which is the defect this whole product exists for.
    for row in reduced.aggregates:
        assert row["method_id"] == "trial.aggregation"
        assert row["quantity"] == "jump_height_from_takeoff_meters"
        # Best of five and best of three are different numbers, so the count travels too.
        assert row["n"] == 2


def test_a_reduction_naming_no_published_rule_is_refused_rather_than_averaged(
    trial_folder, registry_path
):
    """`trial.aggregation` publishes three incompatible rules and none of them is the
    arithmetic mean, so the arithmetic mean is not the near-enough answer to a rule this
    registry does not carry."""
    with pytest.raises(ValueError) as refused:
        run(
            trial_folder,
            registry_path,
            pattern="AT{subject}_{trial}",
            aggregate="arithmetic_mean",
            aggregate_n=2,
        )
    # The refusal repeats the word the caller wrote. A caller cannot correct a word the
    # refusal does not name.
    assert "arithmetic_mean" in str(refused.value)


def test_a_reduction_that_names_a_rule_and_no_count_is_refused_by_name(
    trial_folder, registry_path
):
    """The count is not defaultable: best of five and best of three are different numbers, so
    a rule bound without one would reduce under a count nobody chose."""
    with pytest.raises(ValueError) as refused:
        run(
            trial_folder,
            registry_path,
            pattern="AT{subject}_{trial}",
            aggregate="mean_of_best_two",
            aggregate_ranked_by="reactive_strength_index",
        )
    assert refused.value is not None
