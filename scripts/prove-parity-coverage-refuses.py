#!/usr/bin/env python3
"""Show that the refusals in `result_parity.py` fire, one cause at a time.

A gate whose coverage list was narrower than the document it guarded is what this proves is
fixed, and a proof of that kind is worth nothing unless the refusal is watched failing. Each
case below starts from something that passes, changes one thing, and requires the named
refusal. A case that produced no fault is reported as a case that proved nothing, because a
gate that cannot refuse and a gate with nothing to refuse read identically from the outside.

Three families, each with its own control that has to pass before any case in it counts.

Coverage, over what the surfaces publish. The four answers are the ones the surfaces actually
computed, read from the committed baseline and reshaped into the per-surface documents the
harness collects, so a case is a change against measured input rather than against a document
written here.

The request manifest, over what the population is. Two request files sat in this repository
wired to nothing and no gate said so, which is the defect the population answers; every way
back into it is a case here, including the one that let those two files be forgotten.

The population's coverage of values, over whether any request fills a compared field. Four
surfaces agreeing about an empty list agree about a shape, and the sentence this gate prints
does not say so on its own.

Run it after any edit to `result_parity.py`, and after any change to what a surface publishes:

    python3 scripts/prove-parity-coverage-refuses.py
"""

import json
import pathlib
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import result_parity as gate

ROOT = pathlib.Path(__file__).parent.parent
BASELINE = ROOT / "tests" / "golden" / "result-parity.json"

def answers_that_pass():
    """The four surfaces as they answer today, assembled from the committed result.

    The fields beyond the compared ones are placed from the gate's own register rather than
    from a table here, so this file cannot drift from what the gate expects and then report the
    drift as a case that proved something. A field recorded as agreeing gets one value across
    its carriers, one recorded as disagreeing gets a different value on each, and the surfaces
    are exactly the ones the manifest names.
    """
    document = json.loads(BASELINE.read_text(encoding="utf-8"))
    result = document["result"]
    answers = {surface: dict(result) for surface in gate.surfaces_named_in_manifest()}

    # Published by every surface and covered by an assertion of its own rather than by the
    # comparison. The digest's assertion reads the value, so the four have to match.
    for field in gate.ASSERTED_ANOTHER_WAY:
        for answer in answers.values():
            answer[field] = "content-0"

    for field, declared in gate.SURFACES_THAT_DIFFER.items():
        for surface in declared.carried_by:
            answers[surface][field] = (
                f"{surface}-{field}" if declared.carriers_agree is False else f"one-{field}"
            )
    return answers, document["compared_fields"]


def faults_when(name, change, expected):
    """Apply one change to a passing run and require a fault that names `expected`.

    The register is restored afterwards because one case edits it: a recorded disagreement is
    only repairable by a case that first records one, and leaving that behind would change
    what every case after it is measured against.
    """
    was = dict(gate.SURFACES_THAT_DIFFER)
    answers, fields = answers_that_pass()
    fields = list(fields)
    try:
        fields = change(answers, fields) or fields
        print(f"applied {name}", flush=True)
        faults = gate.coverage_faults(answers, fields)
    finally:
        gate.SURFACES_THAT_DIFFER.clear()
        gate.SURFACES_THAT_DIFFER.update(was)
    hit = [fault for fault in faults if expected in fault]
    if not hit:
        print(f"  NOT REFUSED: no fault mentions {expected!r}", file=sys.stderr)
        for fault in faults:
            print(f"    the faults raised were: {fault}", file=sys.stderr)
        return False
    print(f"  refused: {hit[0].splitlines()[0][:150]}", flush=True)
    return True


def a_control_that_must_pass():
    """The first step everybody skips: the unchanged input has to be green.

    A case that refuses is proof of nothing if the input refuses before it is touched.
    """
    answers, fields = answers_that_pass()
    print("applied nothing, the control", flush=True)
    faults = gate.coverage_faults(answers, fields)
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no case below means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(fields)} compared fields, four surfaces, no fault", flush=True)
    return True


def drop_a_compared_field(answers, fields):
    fields.remove("metrics")
    return fields


def add_a_field_to_every_surface(answers, fields):
    """The defect this gate was blind to: a field reaching all four and named nowhere."""
    for answer in answers.values():
        answer["jump_height_confidence_interval"] = [0.39, 0.43]


def add_a_field_to_one_surface(answers, fields):
    """A result carrying its method on one surface and not the others."""
    answers["python"]["sample_rate_hz_read"] = 1200.0


def move_a_declared_divergence(answers, fields):
    """The browser stops naming the build, so the register's account of it is out of date.

    A divergence that shrinks without disappearing, which is the case the register's set
    comparison exists for. A divergence that spread to every surface trips a different fault,
    below, and one that vanished trips a third.
    """
    answers["browser"].pop("plateforce_version")


def spread_a_divergence_to_every_surface(answers, fields):
    for surface, answer in answers.items():
        answer["descriptions"] = {}


def retire_a_divergence_everywhere(answers, fields):
    for answer in answers.values():
        answer.pop("trial", None)


def compare_a_field_two_surfaces_lack(answers, fields):
    fields.append("spread")
    return fields


def compare_a_field_asserted_another_way(answers, fields):
    fields.append("registry_digest")
    return fields


def make_two_surfaces_read_different_registries(answers, fields):
    answers["r"]["registry_digest"] = "content-somethingelse"


def ask_fewer_surfaces_than_the_manifest_names(answers, fields):
    answers.pop("r")


def stop_publishing_a_field_asserted_another_way(answers, fields):
    for answer in answers.values():
        answer.pop("registry_digest", None)


def make_agreeing_carriers_disagree(answers, fields):
    """A divergence whose carriers stopped matching each other."""
    answers["browser"]["trial"] = "browser-went-its-own-way"


def repair_a_recorded_disagreement(answers, fields):
    """A register entry describing a disagreement that has since been repaired.

    The gate has to refuse the repair as loudly as the break, or an entry sits in the register
    naming a disagreement nobody has any more and nobody moves the field into
    `compared_fields`.

    This used to be written against `registry_version`, the one entry whose carriers
    disagreed. `wsrp/registry-pin` repaired it and took the entry out, and this case was left
    naming a field the register no longer holds, so it stopped refusing and nothing said so:
    the script is run by hand and by no workflow. Written against the register itself now, so
    it cannot go quiet again the next time an entry is discharged. Every entry today records
    agreement or a single carrier, so the disagreement has to be recorded here first.
    """
    entry = gate.SURFACES_THAT_DIFFER["trial"]
    gate.SURFACES_THAT_DIFFER["trial"] = entry._replace(carriers_agree=False)


CASES = [
    ("a name taken out of compared_fields", drop_a_compared_field, "every surface publishes"),
    (
        "a field added to all four surfaces and named nowhere",
        add_a_field_to_every_surface,
        "jump_height_confidence_interval",
    ),
    (
        "a field added to one surface and named nowhere",
        add_a_field_to_one_surface,
        "sample_rate_hz_read",
    ),
    (
        "a declared divergence reaching a surface it is not declared to reach",
        move_a_declared_divergence,
        "declared to reach",
    ),
    (
        "a declared divergence that reached every surface",
        spread_a_divergence_to_every_surface,
        "now reaches every surface",
    ),
    (
        "a declared divergence no surface publishes any more",
        retire_a_divergence_everywhere,
        "no surface publishes it",
    ),
    (
        "a compared field two surfaces do not publish",
        compare_a_field_two_surfaces_lack,
        "cannot be held to one committed value",
    ),
    (
        "a field both compared and declared asserted another way",
        compare_a_field_asserted_another_way,
        "one of the two is wrong",
    ),
    (
        "two surfaces resolving rules out of different registries",
        make_two_surfaces_read_different_registries,
        "different registries",
    ),
    (
        "a run holding fewer surfaces than the manifest names",
        ask_fewer_surfaces_than_the_manifest_names,
        "the manifest names",
    ),
    (
        "the field asserted another way, published by nobody",
        stop_publishing_a_field_asserted_another_way,
        "reads as coverage and covers nothing",
    ),
    (
        "a divergence whose carriers stopped agreeing with each other",
        make_agreeing_carriers_disagree,
        "recorded as agreeing",
    ),
    (
        "a recorded disagreement repaired, which the register has to notice",
        repair_a_recorded_disagreement,
        "recorded as disagreeing",
    ),
]


# The request manifest. A population described wrongly is a gate speaking for requests it
# never asked, which is the same defect one level up from the coverage cases above.

A_POPULATION_THAT_PASSES = [
    "quiet\ttests/golden/result-parity-request.json\ttests/golden/result-parity.json",
    "sentinel\ttests/golden/result-parity-request-sentinel.json\t=quiet",
]


def a_population_on_disk(directory, rows, requests=None, baselines=None):
    """A whole tests/golden of request and baseline files, and a manifest naming them.

    Written out rather than mocked, because the two refusals that matter most read the
    directory rather than the manifest: a request file nobody asks and a record nobody is
    held to are both invisible to anything that only reads the rows.
    """
    root = pathlib.Path(directory)
    golden = root / "tests" / "golden"
    golden.mkdir(parents=True, exist_ok=True)
    for name in requests if requests is not None else [
        "result-parity-request.json",
        "result-parity-request-sentinel.json",
    ]:
        (golden / name).write_text("{}\n", encoding="utf-8")
    for name in baselines if baselines is not None else ["result-parity.json"]:
        (golden / name).write_text("{}\n", encoding="utf-8")
    manifest = root / "result-parity-requests.txt"
    manifest.write_text("# a population\n" + "\n".join(rows) + "\n", encoding="utf-8")

    gate.ROOT = root
    gate.GOLDEN = golden
    gate.REQUEST_MANIFEST = manifest
    return gate.requests_named_in_manifest()


def manifest_faults_when(name, rows, expected, requests=None, baselines=None, writing=False):
    was = (gate.ROOT, gate.GOLDEN, gate.REQUEST_MANIFEST)
    try:
        with tempfile.TemporaryDirectory() as directory:
            parsed = a_population_on_disk(directory, rows, requests, baselines)
            print(f"applied {name}", flush=True)
            faults = gate.request_manifest_faults(parsed, writing=writing)
    finally:
        gate.ROOT, gate.GOLDEN, gate.REQUEST_MANIFEST = was

    hit = [fault for fault in faults if expected in fault]
    if not hit:
        print(f"  NOT REFUSED: no fault mentions {expected!r}", file=sys.stderr)
        for fault in faults:
            print(f"    the faults raised were: {fault}", file=sys.stderr)
        return False
    print(f"  refused: {hit[0][:150]}", flush=True)
    return True


def a_manifest_control_that_must_pass():
    """A population described correctly raises nothing, or no case below means anything."""
    was = (gate.ROOT, gate.GOLDEN, gate.REQUEST_MANIFEST)
    try:
        with tempfile.TemporaryDirectory() as directory:
            parsed = a_population_on_disk(directory, A_POPULATION_THAT_PASSES)
            print("applied nothing, the manifest control", flush=True)
            faults = gate.request_manifest_faults(parsed)
    finally:
        gate.ROOT, gate.GOLDEN, gate.REQUEST_MANIFEST = was
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no manifest case means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(parsed)} requests, one record and one row held to it, no fault")
    return True


MANIFEST_CASES = [
    (
        # The defect that created the population: two request files, wired to nothing, for a
        # day, and every gate in the repository green.
        "a request file on disk that no row of the manifest names",
        dict(
            rows=A_POPULATION_THAT_PASSES,
            requests=[
                "result-parity-request.json",
                "result-parity-request-sentinel.json",
                "result-parity-request-interrupted.json",
            ],
            expected="asks a question and no row of",
        ),
    ),
    (
        "a fifth request added and its baseline forgotten",
        dict(
            rows=A_POPULATION_THAT_PASSES
            + ["gap\ttests/golden/result-parity-request-gap.json\ttests/golden/result-parity-gap.json"],
            requests=[
                "result-parity-request.json",
                "result-parity-request-sentinel.json",
                "result-parity-request-gap.json",
            ],
            expected="there is no such record",
        ),
    ),
    (
        "a row held to nothing at all",
        dict(
            rows=A_POPULATION_THAT_PASSES
            + ["gap\ttests/golden/result-parity-request-gap.json\t"],
            requests=[
                "result-parity-request.json",
                "result-parity-request-sentinel.json",
                "result-parity-request-gap.json",
            ],
            expected="held to no record at all",
        ),
    ),
    (
        "a row naming a request file that is not there",
        dict(
            rows=A_POPULATION_THAT_PASSES
            + ["gap\ttests/golden/result-parity-request-gap.json\t=quiet"],
            expected="and there is no such file",
        ),
    ),
    (
        "a row declared equal to a row that does not exist",
        dict(
            rows=[A_POPULATION_THAT_PASSES[0], A_POPULATION_THAT_PASSES[1].replace("=quiet", "=loud")],
            expected="and no row is named that",
        ),
    ),
    (
        "a row declared equal to itself, which asserts nothing",
        dict(
            rows=[A_POPULATION_THAT_PASSES[0], A_POPULATION_THAT_PASSES[1].replace("=quiet", "=sentinel")],
            expected="declared equal to itself",
        ),
    ),
    (
        "a chain of rows each declared equal to the next",
        dict(
            rows=[
                A_POPULATION_THAT_PASSES[0].replace(
                    "tests/golden/result-parity.json", "=sentinel"
                ),
                A_POPULATION_THAT_PASSES[1],
            ],
            expected="One hop",
        ),
    ),
    (
        "two rows carrying one name",
        dict(
            rows=[A_POPULATION_THAT_PASSES[0], A_POPULATION_THAT_PASSES[0]],
            expected="two rows of the request manifest carry one name",
        ),
    ),
    (
        "a committed record no row is held to",
        dict(
            rows=A_POPULATION_THAT_PASSES,
            baselines=["result-parity.json", "result-parity-gap.json"],
            expected="a committed record no row is held to",
        ),
    ),
    (
        # Regeneration is allowed to write a record that is not there yet, and nothing else.
        "a regeneration that would leave a request file unasked",
        dict(
            rows=A_POPULATION_THAT_PASSES,
            requests=[
                "result-parity-request.json",
                "result-parity-request-sentinel.json",
                "result-parity-request-gap.json",
            ],
            expected="asks a question and no row of",
            writing=True,
        ),
    ),
]


# The population's coverage of values. Three fields were compared and empty in every answer,
# and the gate said four surfaces computed the result.

def hollow_faults_when(name, committed, fields, expected):
    print(f"applied {name}", flush=True)
    faults = gate.hollow_population_faults(committed, fields)
    hit = [fault for fault in faults if expected in fault]
    if not hit:
        print(f"  NOT REFUSED: no fault mentions {expected!r}", file=sys.stderr)
        for fault in faults:
            print(f"    the faults raised were: {fault}", file=sys.stderr)
        return False
    print(f"  refused: {hit[0][:150]}", flush=True)
    return True


def a_population_valuing_everything():
    """Two requests between them putting a value in every compared field.

    `registry_version` is empty in both, as it is in every committed request, and it is the
    one name in `EMPTY_ON_EVERY_REQUEST`. So the control also shows the register covering the
    field it declares rather than merely existing.
    """
    committed = {
        "first": {"metrics": [1.0], "refusals": [], "registry_version": None},
        "second": {"metrics": [], "refusals": [{"code": "no_crossing"}], "registry_version": None},
    }
    fields = {name: sorted(held) for name, held in committed.items()}
    return committed, fields


def a_hollow_control_that_must_pass():
    committed, fields = a_population_valuing_everything()
    print("applied nothing, the hollow control", flush=True)
    faults = gate.hollow_population_faults(committed, fields)
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no hollow case means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print("  passed: every compared field valued by some request, or named as valued by none")
    return True


def a_field_no_request_values():
    committed, fields = a_population_valuing_everything()
    for held in committed.values():
        held["signals"] = []
    return committed, {name: sorted(held) for name, held in committed.items()}


def a_declared_field_some_request_values():
    """The repair the register has to notice, the shape `SURFACES_THAT_DIFFER` already asks.

    A request that fills `registry_version` retires the entry. An entry left behind describes
    a hollow this population no longer has, and a reader would take the printed count on
    trust.
    """
    committed, fields = a_population_valuing_everything()
    committed["second"]["registry_version"] = "2026-07-25"
    return committed, fields


def a_declared_field_nothing_compares():
    committed, fields = a_population_valuing_everything()
    for held in committed.values():
        held.pop("registry_version")
    return committed, {name: sorted(held) for name, held in committed.items()}


HOLLOW_CASES = [
    (
        "a compared field no request in the population puts a value in",
        a_field_no_request_values,
        "no committed request puts a value in it",
    ),
    (
        "a field named as valued by nobody that some request values",
        a_declared_field_some_request_values,
        "so the entry is out of date",
    ),
    (
        "a field named as valued by nobody that no request compares at all",
        a_declared_field_nothing_compares,
        "reads as coverage and covers nothing",
    ),
]


def main():
    survived = []

    if not a_control_that_must_pass():
        raise SystemExit(1)
    survived += [name for name, change, expected in CASES if not faults_when(name, change, expected)]

    print()
    if not a_manifest_control_that_must_pass():
        raise SystemExit(1)
    for name, case in MANIFEST_CASES:
        if not manifest_faults_when(name, **case):
            survived.append(name)

    print()
    if not a_hollow_control_that_must_pass():
        raise SystemExit(1)
    for name, build, expected in HOLLOW_CASES:
        committed, fields = build()
        if not hollow_faults_when(name, committed, fields, expected):
            survived.append(name)

    total = len(CASES) + len(MANIFEST_CASES) + len(HOLLOW_CASES)
    print()
    print(f"{total - len(survived)} of {total} cases were refused")
    if survived:
        for name in survived:
            print(f"plateforce: {name} did not make the gate refuse", file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
