#!/usr/bin/env python3
"""Show that the refusals in `result_parity.py` fire, one cause at a time.

A gate whose coverage list is narrower than the document it guards refuses nothing, and a
proof of that kind is worth nothing unless the refusal is watched failing. Each case below
starts from something that passes, changes one thing, and requires the named refusal. A case that produced no fault is reported as a case that proved nothing, because a
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
SWEEP_BASELINE = ROOT / "tests" / "golden" / "result-parity-sweep.json"


def answers_from_baseline(baseline, kind, asked):
    """The surfaces as they answer today, assembled from a committed result.

    The fields beyond the compared ones are placed from the gate's own register rather than
    from a table here, so this file cannot drift from what the gate expects and then report the
    drift as a case that proved something. A field recorded as agreeing gets one value across
    its carriers, one recorded as disagreeing gets a different value on each, and the surfaces
    are exactly the ones that request is asked of.
    """
    document = json.loads(baseline.read_text(encoding="utf-8"))
    result = document["result"]
    answers = {surface: dict(result) for surface in asked}

    # Published by every surface and covered by an assertion of its own rather than by the
    # comparison. Each assertion reads the value, so the surfaces have to match.
    for field in gate.ASSERTED_ANOTHER_WAY[kind]:
        for answer in answers.values():
            answer[field] = "content-0"

    for field, declared in gate.SURFACES_THAT_DIFFER[kind].items():
        for surface in declared.carried_by:
            answers[surface][field] = (
                f"{surface}-{field}" if declared.carriers_agree is False else f"one-{field}"
            )
    return answers, document["compared_fields"]


def answers_that_pass():
    return answers_from_baseline(BASELINE, gate.ANALYSED, gate.surfaces_named_in_manifest())


def swept_answers_that_pass():
    """The sweep, over the surfaces that can be asked one.

    A separate control because the register is per kind: an analysed document carries
    `plateforce_version` on two surfaces and a swept one on every surface that answers, so a
    case written against one says nothing about the other.
    """
    asked = gate.surfaces_named_in_manifest() - set(gate.SURFACES_NOT_ASKED[gate.SWEPT])
    return answers_from_baseline(SWEEP_BASELINE, gate.SWEPT, asked), asked


def faults_when(name, change, expected, kind=None, build=None):
    """Apply one change to a passing run and require a fault that names `expected`.

    The register is restored afterwards because one case edits it: a recorded disagreement is
    only repairable by a case that first records one, and leaving that behind would change
    what every case after it is measured against.
    """
    kind = kind or gate.ANALYSED
    was = dict(gate.SURFACES_THAT_DIFFER[kind])
    if build is None:
        answers, fields = answers_that_pass()
        asked = gate.surfaces_named_in_manifest()
    else:
        (answers, fields), asked = build()
    fields = list(fields)
    try:
        fields = change(answers, fields) or fields
        print(f"applied {name}", flush=True)
        faults = gate.coverage_faults(answers, fields, kind, asked)
    finally:
        gate.SURFACES_THAT_DIFFER[kind].clear()
        gate.SURFACES_THAT_DIFFER[kind].update(was)
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
    faults = gate.coverage_faults(answers, fields, gate.ANALYSED, gate.surfaces_named_in_manifest())
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no case below means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(fields)} compared fields, four surfaces, no fault", flush=True)
    return True


def a_swept_control_that_must_pass():
    (answers, fields), asked = swept_answers_that_pass()
    print("applied nothing, the swept control", flush=True)
    faults = gate.coverage_faults(answers, fields, gate.SWEPT, asked)
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no swept case means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(fields)} compared fields, {len(asked)} surfaces, no fault", flush=True)
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

    Written against the register itself rather than a named field: a case naming one field
    goes quiet the moment that entry is discharged, and this script is run by hand and by no
    workflow. Every entry today records agreement or a single carrier, so the disagreement has
    to be recorded here first.
    """
    entry = gate.SURFACES_THAT_DIFFER[gate.ANALYSED]["trial"]
    gate.SURFACES_THAT_DIFFER[gate.ANALYSED]["trial"] = entry._replace(carriers_agree=False)


def make_two_surfaces_name_different_builds(answers, fields):
    """A sweep leaving two surfaces built out of step, which no committed value can see.

    `plateforce_version` is compared on no request: an analysed document carries it on two
    surfaces and a swept one moves with every release, so both are asserted rather than held
    to a record. This is what asserting it has to catch.
    """
    answers["python"]["plateforce_version"] = "0.0.0-somewhere-else"


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
        "a run holding fewer surfaces than the request is asked of",
        ask_fewer_surfaces_than_the_manifest_names,
        "nothing below speaks for the surfaces it claims",
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


# The sweep, whose register is a second one. Every case above is written against an analysed
# document, and a swept document is where this project's founding measurement lives.
SWEPT_CASES = [
    (
        "two surfaces reporting one sweep from builds that are not the same build",
        make_two_surfaces_name_different_builds,
        "different builds",
    ),
    (
        "a field one surface publishes on a sweep and the others do not",
        add_a_field_to_one_surface,
        "sample_rate_hz_read",
    ),
]


# The request manifest. A population described wrongly is a gate speaking for requests it
# never asked, which is the same defect one level up from the coverage cases above.

def every_surface():
    return ",".join(sorted(gate.surfaces_named_in_manifest()))


def every_surface_a_sweep_reaches():
    """The surfaces a swept request is asked of, derived rather than written out.

    A surface added to the manifest, or one whose entry point gains the ability to state a
    sweep, changes this population. Written here as a list it would go stale silently and the
    control would start failing for a reason that is not a defect.
    """
    reached = gate.surfaces_named_in_manifest() - set(gate.SURFACES_NOT_ASKED[gate.SWEPT])
    return ",".join(sorted(reached))


# Three rows, because the register holds an entry whose account of itself rests on a swept
# request being in the population. A population of analysed requests alone makes that entry
# refuse, which is what it is there to do.
A_POPULATION_THAT_PASSES = [
    f"quiet\ttests/golden/result-parity-request.json\ttests/golden/result-parity.json\t{every_surface()}",
    f"sentinel\ttests/golden/result-parity-request-sentinel.json\t=quiet\t{every_surface()}",
    f"sweep\ttests/golden/result-parity-request-sweep.json\ttests/golden/result-parity-sweep.json\t{every_surface_a_sweep_reaches()}",
]

DEFAULT_REQUESTS = [
    "result-parity-request.json",
    "result-parity-request-sentinel.json",
    "result-parity-request-sweep.json",
]
DEFAULT_BASELINES = ["result-parity.json", "result-parity-sweep.json"]


def a_population_on_disk(directory, rows, requests=None, baselines=None):
    """A whole tests/golden of request and baseline files, and a manifest naming them.

    Written out rather than mocked, because the two refusals that matter most read the
    directory rather than the manifest: a request file nobody asks and a record nobody is
    held to are both invisible to anything that only reads the rows. A file whose name carries
    `sweep` is written with a sweep block, because the gate reads the kind of a question off
    the file that asks it.
    """
    root = pathlib.Path(directory)
    golden = root / "tests" / "golden"
    golden.mkdir(parents=True, exist_ok=True)
    for name in requests if requests is not None else DEFAULT_REQUESTS:
        body = '{"sweep": {"slots": []}}\n' if "sweep" in name else "{}\n"
        (golden / name).write_text(body, encoding="utf-8")
    for name in baselines if baselines is not None else DEFAULT_BASELINES:
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


GAP = "gap\ttests/golden/result-parity-request-gap.json"
WITH_A_GAP = DEFAULT_REQUESTS + ["result-parity-request-gap.json"]
ANALYSED_ROWS = [A_POPULATION_THAT_PASSES[0], A_POPULATION_THAT_PASSES[1]]
SWEEP_ROW = A_POPULATION_THAT_PASSES[2]

MANIFEST_CASES = [
    (
        # The defect that created the population: two request files, wired to nothing, for a
        # day, and every gate in the repository green.
        "a request file on disk that no row of the manifest names",
        dict(
            rows=A_POPULATION_THAT_PASSES,
            requests=DEFAULT_REQUESTS + ["result-parity-request-interrupted.json"],
            expected="asks a question and no row of",
        ),
    ),
    (
        "a further request added and its baseline forgotten",
        dict(
            rows=A_POPULATION_THAT_PASSES
            + [f"{GAP}\ttests/golden/result-parity-gap.json\t{every_surface()}"],
            requests=WITH_A_GAP,
            expected="there is no such record",
        ),
    ),
    (
        "a row held to nothing at all",
        dict(
            rows=A_POPULATION_THAT_PASSES + [f"{GAP}\t\t{every_surface()}"],
            requests=WITH_A_GAP,
            expected="held to no record at all",
        ),
    ),
    (
        "a row naming a request file that is not there",
        dict(
            rows=A_POPULATION_THAT_PASSES + [f"{GAP}\t=quiet\t{every_surface()}"],
            expected="and there is no such file",
        ),
    ),
    (
        "a row declared equal to a row that does not exist",
        dict(
            rows=[ANALYSED_ROWS[0], ANALYSED_ROWS[1].replace("=quiet", "=loud"), SWEEP_ROW],
            expected="and no row is named that",
        ),
    ),
    (
        "a row declared equal to itself, which asserts nothing",
        dict(
            rows=[ANALYSED_ROWS[0], ANALYSED_ROWS[1].replace("=quiet", "=sentinel"), SWEEP_ROW],
            expected="declared equal to itself",
        ),
    ),
    (
        "a chain of rows each declared equal to the next",
        dict(
            rows=[
                ANALYSED_ROWS[0].replace("tests/golden/result-parity.json", "=sentinel"),
                ANALYSED_ROWS[1],
                SWEEP_ROW,
            ],
            expected="One hop",
        ),
    ),
    (
        "two rows carrying one name",
        dict(
            rows=[ANALYSED_ROWS[0], ANALYSED_ROWS[0], SWEEP_ROW],
            expected="two rows of the request manifest carry one name",
        ),
    ),
    (
        "a committed record no row is held to",
        dict(
            rows=A_POPULATION_THAT_PASSES,
            baselines=DEFAULT_BASELINES + ["result-parity-gap.json"],
            expected="a committed record no row is held to",
        ),
    ),
    (
        # Regeneration is allowed to write a record that is not there yet, and nothing else.
        "a regeneration that would leave a request file unasked",
        dict(
            rows=A_POPULATION_THAT_PASSES,
            requests=WITH_A_GAP,
            expected="asks a question and no row of",
            writing=True,
        ),
    ),
    (
        "a row asked of a surface no manifest names",
        dict(
            rows=[f"{ANALYSED_ROWS[0]},abacus", ANALYSED_ROWS[1], SWEEP_ROW],
            expected="names no such surface",
        ),
    ),
    (
        # A population of one agrees with itself, which is the shape this project keeps
        # catching one level down, in a divergence carried by a single surface.
        "a request one surface answers",
        dict(
            rows=[
                ANALYSED_ROWS[0].rsplit("\t", 1)[0] + "\tcli",
                ANALYSED_ROWS[1],
                SWEEP_ROW,
            ],
            expected="agreeing with itself",
        ),
    ),
    (
        "a surface left off a request and named nowhere",
        dict(
            rows=[
                ANALYSED_ROWS[0].replace(every_surface(), every_surface_a_sweep_reaches()),
                ANALYSED_ROWS[1],
                SWEEP_ROW,
            ],
            expected="nothing here says why",
        ),
    ),
    (
        "a listed surface no request asks at all",
        dict(
            rows=[
                row.replace(every_surface(), every_surface_a_sweep_reaches())
                for row in A_POPULATION_THAT_PASSES
            ],
            expected="and no request asks it",
        ),
    ),
    (
        "a surface recorded as unable to answer a sweep, answering one",
        dict(
            rows=ANALYSED_ROWS
            + [SWEEP_ROW.replace(every_surface_a_sweep_reaches(), every_surface())],
            expected="so the entry is out of date and the surface answers for real",
        ),
    ),
    (
        # Two refusals over one population, because a swept request leaving the population
        # takes two accounts with it: the surface that cannot be asked one, and the register
        # entry that says the sweep is compared elsewhere.
        "a population holding no swept request, with a surface recorded as unable to answer one",
        dict(
            rows=ANALYSED_ROWS,
            requests=DEFAULT_REQUESTS[:2],
            baselines=DEFAULT_BASELINES[:1],
            expected="reads as coverage and covers nothing",
        ),
    ),
    (
        "a register entry resting on a swept request the population no longer holds",
        dict(
            rows=ANALYSED_ROWS,
            requests=DEFAULT_REQUESTS[:2],
            baselines=DEFAULT_BASELINES[:1],
            expected="its account of itself is a sentence nothing measures",
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
    if not a_swept_control_that_must_pass():
        raise SystemExit(1)
    survived += [
        name
        for name, change, expected in SWEPT_CASES
        if not faults_when(
            name, change, expected, kind=gate.SWEPT, build=swept_answers_that_pass
        )
    ]

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

    total = len(CASES) + len(SWEPT_CASES) + len(MANIFEST_CASES) + len(HOLLOW_CASES)
    print()
    print(f"{total - len(survived)} of {total} cases were refused")
    if survived:
        for name in survived:
            print(f"plateforce: {name} did not make the gate refuse", file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
