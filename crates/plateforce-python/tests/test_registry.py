"""Selecting a method, and what the entry has to show before you can."""

import pathlib
import shutil

import pytest

import plateforce as pf

def _find_shipped_registry():
    """Locate `registry/` by walking up from this file.

    The tests run from the repository, from an extracted source distribution and from a
    wheel test directory, and the registry is only present in the first. Absent is a skip,
    never a collection error.
    """
    for ancestor in pathlib.Path(__file__).resolve().parents:
        candidate = ancestor / "registry"
        if (candidate / "constructs.toml").is_file():
            return candidate
    return None


SHIPPED_REGISTRY = _find_shipped_registry()


def test_the_census_reports_populations_separately(registry):
    census = registry.census
    assert census.constructs == 5
    assert census.computation_entries == 8
    assert census.protocol_entries == 0
    assert not hasattr(census, "total"), "the two populations are never summed"


def test_an_entry_carries_its_rule_status_and_citations(registry):
    entry = registry.method("onset.threshold.noise_relative")
    assert entry.status == "accepted"
    assert entry.confidence == "high"
    assert entry.debate == "genuine"
    assert "k standard deviations" in entry.rule
    assert [citation.key for citation in entry.citations] == ["owen2014"]
    assert entry.citations[0].role == "proposes"
    assert entry.citations[0].obtained is True


def test_a_bias_cannot_be_read_without_its_criterion(registry):
    bias = registry.method("onset.threshold.noise_relative").biases[0]
    assert bias.criterion == "onset.manual_visual.tillin2010"
    assert bias.criterion_kind == "human_visual"
    assert bias.conditional_on_success is True
    assert "conditional" in repr(bias)


def test_a_failure_block_is_visible_on_the_entry_and_in_its_repr(registry):
    entry = registry.method("onset.threshold.noise_relative_forward_offset")
    failure = entry.failure
    assert failure is not None
    assert failure.numerator == 36
    assert failure.denominator == 241
    assert failure.rate == pytest.approx(0.1494)
    assert failure.detectability == "silent"
    assert "FAILS" in repr(entry), "a rule that finds the wrong event must not look ordinary"


def test_entries_without_a_failure_block_report_none(registry):
    assert registry.method("bwepoch.fixed_window").failure is None


def test_methods_that_can_fail_can_be_listed_before_one_is_chosen(registry):
    ids = [entry.id for entry in registry.methods_that_can_fail()]
    assert ids == ["onset.threshold.noise_relative_forward_offset"]


def test_genuine_debates_are_separable_from_the_rest(registry):
    ids = [entry.id for entry in registry.genuine_debates()]
    assert ids == ["onset.threshold.noise_relative"]


def test_the_entry_says_whether_this_build_can_run_it(registry):
    assert registry.method("onset.threshold.noise_relative").implemented is True
    assert registry.method("onset.threshold.noise_relative_forward_offset").implemented is False
    implemented = {entry.id for entry in registry.implemented_methods()}
    assert "onset.threshold.noise_relative_forward_offset" not in implemented


def test_binding_records_the_registry_default_and_says_it_defaulted(registry):
    bound = registry.method("bwepoch.fixed_window").bind()
    assert bound.parameters == {"duration": 1.0}
    assert bound.defaulted_parameters == ["duration"]


def test_binding_a_value_the_literature_lacks_is_allowed_and_reported(registry):
    bound = registry.method("onset.threshold.noise_relative").bind(k=7.0)
    assert bound.parameters == {"k": 7.0}
    assert bound.unpublished_parameters == ["k"]
    assert registry.method("onset.threshold.noise_relative").bind(k=5.0).unpublished_parameters == []


def test_an_unknown_parameter_names_the_method_and_what_is_on_offer(registry):
    with pytest.raises(pf.ParameterError) as raised:
        registry.method("onset.threshold.noise_relative").bind(kk=5.0)
    message = str(raised.value)
    assert "onset.threshold.noise_relative" in message
    assert "kk" in message
    assert "k" in message
    assert raised.value.method_id == "onset.threshold.noise_relative"
    assert raised.value.parameter == "kk"


def test_a_non_numeric_parameter_value_is_refused(registry):
    with pytest.raises(pf.ParameterError) as raised:
        registry.method("onset.threshold.noise_relative").bind(k="five")
    assert "must be a number" in str(raised.value)


def test_a_non_finite_parameter_value_is_refused(registry):
    with pytest.raises(pf.ParameterError) as raised:
        registry.method("onset.threshold.noise_relative").bind(k=float("inf"))
    assert "finite" in str(raised.value)


def test_binding_is_order_independent_so_two_bindings_fingerprint_alike(registry):
    entry = registry.method("takeoff.threshold.absolute_force")
    first = entry.bind(threshold_n=20.0, persistence_ms=30.0)
    second = entry.bind(persistence_ms=30.0, threshold_n=20.0)
    assert list(first.parameters.items()) == list(second.parameters.items())


def test_an_unknown_method_id_is_an_error_naming_the_registry_size(registry):
    with pytest.raises(pf.MethodError) as raised:
        registry.method("onset.threshold.nonexistent")
    assert "onset.threshold.nonexistent" in str(raised.value)


def test_a_registry_that_fails_validation_does_not_load_at_all(tmp_path):
    (tmp_path / "constructs.toml").write_text("")
    (tmp_path / "methods").mkdir()
    (tmp_path / "methods" / "broken.toml").write_text(
        '[[method]]\n'
        'id = "onset.orphan"\n'
        'construct = "no_such_construct"\n'
        'title = "Orphan"\n'
        'rule = "Nothing."\n'
        'status = "accepted"\n'
        'confidence = "high"\n'
    )
    with pytest.raises(pf.RegistryError) as raised:
        pf.Registry.load(tmp_path)
    assert "no_such_construct" in str(raised.value)


def test_a_pinned_registry_reports_the_pin_and_an_unpinned_one_reports_nothing(
    registry, registry_path
):
    assert registry.version == "fixture-1"
    assert pf.Registry.load(registry_path).version is None


def test_a_registry_names_itself_by_its_files_whether_or_not_it_was_pinned(
    registry, registry_path
):
    unpinned = pf.Registry.load(registry_path)
    assert unpinned.digest.startswith("content-")
    assert unpinned.digest == registry.digest, "the pin must not change what was loaded"
    assert unpinned.digest in repr(unpinned)


def test_the_digest_changes_with_the_registry_content_and_not_otherwise(
    tmp_path, registry_path
):
    copied = tmp_path / "registry"
    shutil.copytree(registry_path, copied)

    first = pf.Registry.load(copied).digest
    assert pf.Registry.load(copied).digest == first

    seed = copied / "methods" / "seed.toml"
    seed.write_text(seed.read_text().replace("first sample departing", "first sample leaving"))
    assert pf.Registry.load(copied).digest != first


@pytest.fixture(scope="session")
def shipped_registry():
    """The registry this repository ships, as opposed to the fixture above.

    A failure here is a registry problem, not a binding problem, and the two are worth
    telling apart at a glance.
    """
    if SHIPPED_REGISTRY is None:
        pytest.skip("shipped registry not present, which is expected outside the repository")
    try:
        return pf.Registry.load(SHIPPED_REGISTRY)
    except pf.RegistryError as error:
        pytest.fail(
            f"the registry at {SHIPPED_REGISTRY} does not load, so the package cannot be "
            f"used against it:\n{error}"
        )


def _digest_computed_here(root):
    """FNV-1a over path and contents, in path order, written out rather than called.

    Reading a digest back from the package it came from proves nothing about it. This
    walks the same files and does the same arithmetic independently, so agreement between
    the two is evidence and not an echo.
    """
    digest = 0xCBF29CE484222325
    for relative in sorted(path.relative_to(root).as_posix() for path in root.rglob("*.toml")):
        for byte in relative.encode() + (root / relative).read_bytes():
            digest ^= byte
            digest = (digest * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"content-{digest:016x}"


def test_the_shipped_registry_names_itself_by_its_own_bytes(shipped_registry):
    assert shipped_registry.digest == _digest_computed_here(SHIPPED_REGISTRY)


def test_the_shipped_registry_carries_the_ids_the_analysis_dispatches_on(shipped_registry):
    ids = set(shipped_registry.method_ids())
    for required in (
        "bwepoch.fixed_window",
        "onset.threshold.noise_relative",
        "takeoff.threshold.absolute_force",
    ):
        assert required in ids, f"the analysis dispatches on '{required}' and it is gone"
        assert shipped_registry.method(required).implemented
