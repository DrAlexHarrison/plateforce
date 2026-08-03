"""Reading a trace in, and what the trial declares about what it read."""

import array

import numpy as np
import pytest

import plateforce as pf

from conftest import SAMPLE_RATE_HZ


def test_numpy_array_round_trips_unchanged(force_newtons):
    trial = pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ)
    assert len(trial) == len(force_newtons)
    assert trial.sample_count == len(force_newtons)
    np.testing.assert_array_equal(np.asarray(trial.force_newtons), force_newtons)


def test_the_buffer_path_and_the_sequence_path_agree():
    values = [1.5, 2.5, 3.5, 4.0]
    from_list = pf.Trial(values, sample_rate_hz=100.0)
    from_numpy = pf.Trial(np.asarray(values, dtype=np.float64), sample_rate_hz=100.0)
    from_array = pf.Trial(array.array("d", values), sample_rate_hz=100.0)
    assert from_list.force_newtons == from_numpy.force_newtons == from_array.force_newtons


def test_a_memoryview_and_a_strided_view_are_read_correctly():
    values = np.asarray([1.0, 9.0, 2.0, 9.0, 3.0], dtype=np.float64)
    assert pf.Trial(memoryview(values), sample_rate_hz=10.0).force_newtons == list(values)
    assert pf.Trial(values[::2], sample_rate_hz=10.0).force_newtons == [1.0, 2.0, 3.0]


def test_a_narrower_dtype_is_refused_rather_than_widened_silently():
    with pytest.raises(pf.TrialError) as raised:
        pf.Trial(np.asarray([1.0, 2.0], dtype=np.float32), sample_rate_hz=100.0)
    message = str(raised.value)
    assert "float32" in message
    assert "astype" in message


def test_a_two_dimensional_array_is_refused():
    with pytest.raises(pf.TrialError) as raised:
        pf.Trial(np.zeros((4, 2), dtype=np.float64), sample_rate_hz=100.0)
    assert "dimension" in str(raised.value)


def test_an_empty_trace_is_refused_rather_than_returning_a_number():
    with pytest.raises(pf.TrialError):
        pf.Trial(np.zeros(0, dtype=np.float64), sample_rate_hz=100.0)


def test_a_bad_sample_rate_names_the_value():
    with pytest.raises(pf.TrialError) as raised:
        pf.Trial([1.0, 2.0], sample_rate_hz=0.0)
    assert "sample rate" in str(raised.value)


def test_acquisition_is_incomplete_until_every_member_is_present():
    partial = pf.Acquisition(filter_at_capture="none", tare_state="tared_before_trial")
    assert not partial.is_complete
    assert "firmware_version" in partial.missing
    assert "floor_surface" in partial.missing


def test_a_complete_acquisition_block_makes_the_trial_comparable(force_newtons, complete_acquisition):
    assert complete_acquisition.is_complete
    assert complete_acquisition.missing == []
    trial = pf.Trial(
        force_newtons, sample_rate_hz=SAMPLE_RATE_HZ, acquisition=complete_acquisition
    )
    assert trial.acquisition_complete
    assert pf.Trial(force_newtons, sample_rate_hz=SAMPLE_RATE_HZ).acquisition_complete is False


def test_a_declared_sentinel_is_reported_and_the_samples_are_left_in_place():
    values = [600.0, 0.0, 601.0, 0.0, 602.0]
    trial = pf.Trial(values, sample_rate_hz=100.0, sentinel=pf.Sentinel.zero())
    assert trial.sample_count == 5, "a trace must not lose samples: the time base would shift"
    assert trial.exclusions.dropped_samples == 2
    assert trial.exclusions.sentinel_convention == "zero"
    assert "shift the time base" in trial.exclusions.reason


def test_non_finite_samples_are_reported_without_a_declared_convention():
    trial = pf.Trial([600.0, float("nan"), 602.0], sample_rate_hz=100.0)
    assert trial.exclusions.dropped_samples == 1
    assert trial.exclusions.sentinel_convention is None


def test_partitioning_a_result_column_reports_what_it_dropped():
    partition = pf.partition_sentinel_values([45.0, 0.0, 51.0, 0.0], pf.Sentinel.zero())
    assert partition.kept == [45.0, 51.0]
    assert partition.dropped_indices == [1, 3]
    assert partition.exclusions.dropped_samples == 2
    assert partition.exclusions.sentinel_convention == "zero"


def test_sentinel_conventions_are_distinct():
    values = [45.0, 0.0, -1.0]
    assert pf.partition_sentinel_values(values, pf.Sentinel.zero()).kept == [45.0, -1.0]
    assert pf.partition_sentinel_values(values, pf.Sentinel.negative_one()).kept == [45.0, 0.0]
    assert pf.partition_sentinel_values(values, pf.Sentinel.value(45.0)).kept == [0.0, -1.0]


def test_time_at_refuses_an_index_past_the_end(trial):
    assert trial.time_at(1200) == pytest.approx(1.0)
    with pytest.raises(pf.TrialError):
        trial.time_at(trial.sample_count)


# Reading a file. Until this existed a notebook parsed the export itself, which put the
# delimiter, the column and the missing-sample convention in a script nothing records, and
# the result then carried a provenance chain resting on three choices no reader can recover.


def a_three_column_export(directory, rows, delimiter="\t"):
    """Time, a second channel, then force, so a test reading the wrong column reads a number
    rather than failing to parse and passing for the wrong reason."""
    path = directory / "trial.txt"
    path.write_text(
        "".join(
            delimiter.join([f"{index / 100:.4f}", "0.0", f"{value:.4f}"]) + "\n"
            for index, value in enumerate(rows)
        )
    )
    return path


def test_a_file_and_the_same_numbers_handed_in_as_an_array_give_one_trace(tmp_path):
    rows = [600.0, 601.5, 599.25, 602.0]
    path = a_three_column_export(tmp_path, rows)
    from_file = pf.read_force_file(path, sample_rate_hz=100.0, delimiter="\t", force_column=2)
    assert from_file.force_newtons == pf.Trial(rows, sample_rate_hz=100.0).force_newtons


def test_the_read_report_names_every_choice_the_read_rested_on(tmp_path):
    path = a_three_column_export(tmp_path, [600.0, 601.0])
    report = pf.read_force_file(
        path, sample_rate_hz=100.0, delimiter="\t", force_column=2
    ).read_report
    assert report.delimiter == "\t"
    assert report.force_column == 2
    assert report.rows_read == 2
    assert report.columns_per_row == 3
    assert report.blank_lines_skipped == 0
    assert report.source == str(path)


def test_a_trace_handed_in_as_an_array_reports_no_read():
    assert pf.Trial([1.0, 2.0], sample_rate_hz=100.0).read_report is None


def test_the_column_asked_for_is_the_column_read(tmp_path):
    path = a_three_column_export(tmp_path, [600.0, 601.0])
    forces = pf.read_force_file(path, sample_rate_hz=100.0, delimiter="\t", force_column=2)
    others = pf.read_force_file(path, sample_rate_hz=100.0, delimiter="\t", force_column=1)
    assert forces.force_newtons == [600.0, 601.0]
    assert others.force_newtons == [0.0, 0.0]


def test_a_column_that_is_not_there_is_refused_naming_the_index_it_wanted(tmp_path):
    path = a_three_column_export(tmp_path, [600.0])
    with pytest.raises(pf.PlateforceError) as raised:
        pf.read_force_file(path, sample_rate_hz=100.0, delimiter="\t", force_column=7)
    assert "7" in str(raised.value)


def test_a_file_that_is_not_there_refuses_under_the_code_a_caller_branches_on(tmp_path):
    with pytest.raises(pf.PlateforceError) as raised:
        pf.read_force_file(
            tmp_path / "absent.txt", sample_rate_hz=100.0, delimiter="\t", force_column=0
        )
    assert raised.value.code == "file_not_read"


def test_a_separator_of_several_characters_is_refused_rather_than_read_as_its_first(tmp_path):
    path = a_three_column_export(tmp_path, [600.0])
    with pytest.raises(pf.ParameterError) as raised:
        pf.read_force_file(path, sample_rate_hz=100.0, delimiter="\t\t", force_column=2)
    assert raised.value.parameter == "delimiter"


def test_the_rate_the_column_and_the_separator_have_no_defaults(tmp_path):
    """A rate that is guessed scales every velocity, displacement and impulse with it, and a
    guessed column can be the wrong one quietly, so each has to be stated."""
    path = a_three_column_export(tmp_path, [600.0])
    for omitted in ("sample_rate_hz", "delimiter", "force_column"):
        stated = {"sample_rate_hz": 100.0, "delimiter": "\t", "force_column": 2}
        del stated[omitted]
        with pytest.raises(TypeError) as raised:
            pf.read_force_file(path, **stated)
        assert omitted in str(raised.value)


def test_a_sentinel_declared_on_a_read_is_reported(tmp_path):
    path = a_three_column_export(tmp_path, [600.0, 0.0, 602.0])
    trial = pf.read_force_file(
        path, sample_rate_hz=100.0, delimiter="\t", force_column=2, sentinel=pf.Sentinel.zero()
    )
    assert trial.sample_count == 3
    assert trial.exclusions.dropped_samples == 1
    assert trial.exclusions.sentinel_convention == "zero"


def test_a_comma_separated_export_reads(tmp_path):
    path = a_three_column_export(tmp_path, [600.0, 601.0], delimiter=",")
    trial = pf.read_force_file(path, sample_rate_hz=100.0, delimiter=",", force_column=2)
    assert trial.force_newtons == [600.0, 601.0]
