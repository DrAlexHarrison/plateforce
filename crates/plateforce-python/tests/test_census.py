"""Every population the registry carries, counted here and counted the same as the terminal.

A count reported without its denominator, or a population that exists and is reported by one
surface and not another, is the defect this software is built against. Python reported three
where the terminal reported four, so a notebook reader could not see that presets existed at
all.
"""

import json
import os
import shutil
import subprocess

import pytest

import plateforce as pf

REPOSITORY = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))

# The four the registry declares. Named here rather than read off the object, so a population
# the registry gains and this surface silently stops reporting fails rather than shrinking a
# set compared with itself.
POPULATIONS = ("constructs", "computation_entries", "protocol_entries", "preset_entries")


@pytest.fixture
def shipped():
    return pf.Registry.load()


def terminal_census():
    if shutil.which("cargo") is None:
        pytest.skip("no cargo on this machine, so the terminal cannot be asked")
    finished = subprocess.run(
        ["cargo", "run", "-q", "-p", "plateforce-cli", "--", "--format", "json",
         "registry", "census"],
        cwd=REPOSITORY, capture_output=True, text=True,
    )
    if finished.returncode != 0:
        pytest.skip(f"the terminal could not be built here: {finished.stderr[-300:]}")
    return json.loads(finished.stdout)["ok"]


def test_the_notebook_counts_the_populations_the_terminal_counts(shipped):
    there = terminal_census()
    for population in POPULATIONS:
        assert getattr(shipped.census, population) == there[population], population


def test_every_population_is_reported(shipped):
    for population in POPULATIONS:
        assert isinstance(getattr(shipped.census, population), int), population
    assert shipped.census.preset_entries > 0, "the registry ships published pipelines"


def test_the_populations_are_never_summed(shipped):
    """Two of these are not two of anything: a construct and a rule that fills it are
    different kinds. A total would invite a reader to quote one number for the registry."""
    for forbidden in ("total", "entries", "count"):
        assert not hasattr(shipped.census, forbidden), forbidden
    assert all(population in repr(shipped.census) for population in POPULATIONS)


def test_the_revision_the_registry_declares_reaches_a_reader(shipped):
    """Distinct from the revision a caller pinned. Nothing on this surface reported it, so a
    notebook was the one surface unable to say which revision of the data produced a number."""
    assert shipped.version is None, "nothing was pinned by this call"
    assert shipped.declared_version, "the shipped registry names a revision"


def test_a_pinned_revision_and_a_declared_one_are_two_facts(shipped):
    pinned = pf.Registry.load(version="pinned-by-this-test")
    assert pinned.version == "pinned-by-this-test"
    assert pinned.declared_version == shipped.declared_version
    assert pinned.version != pinned.declared_version


def test_the_digest_names_the_bytes_rather_than_the_revision(shipped):
    """A declared revision cannot promise what a digest measures: two registries differing by
    one edited rule declare the same revision and differ here."""
    assert shipped.digest.startswith("content-")
    assert shipped.digest != shipped.declared_version
