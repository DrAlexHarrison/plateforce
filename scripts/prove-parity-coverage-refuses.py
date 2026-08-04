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

The record of divergent values, over the fields no projection can hold. A field two surfaces
carry cannot go in the result every surface is held to, and leaving it out of the record left
the carriers agreeing with each other and compared with nothing.

The population's coverage of values, over whether any request fills a compared field. Four
surfaces agreeing about an empty list agree about a shape, and the sentence this gate prints
does not say so on its own.

Run it after any edit to `result_parity.py`, and after any change to what a surface publishes:

    python3 scripts/prove-parity-coverage-refuses.py
"""

import copy
import json
import pathlib
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import result_parity as gate

ROOT = pathlib.Path(__file__).parent.parent
BASELINE = ROOT / "tests" / "golden" / "result-parity.json"
PLATE_BASELINE = ROOT / "tests" / "golden" / "result-parity-plate.json"
SWEEP_BASELINE = ROOT / "tests" / "golden" / "result-parity-sweep.json"

# Which request each control is assembled from, named because the reach register is keyed by
# request as well as by kind. A control built for one request and measured as another places a
# field the request leaves off every wire, or leaves off one its answers carry, and either way
# the control refuses before a case has touched it.
QUIET = "quiet"
PLATE = "plate"
SWEEP = "sweep"


# What every surface publishes for a field asserted another way, when they agree. One entry
# per field the gate asserts, and a field with no entry stops this script rather than being
# stood in for.
#
# The assertions read the value, so the shape is part of what makes a case evidence. A block
# keyed by quantity stood in for by a string does not compare as agreement or as disagreement:
# it raises inside the assertion, and a script that dies on its own control proves nothing at
# all. Which is what `descriptions` did here the hour it became the third such field.
AN_AGREEING_VALUE = {
    "registry_digest": "content-0",
    "registry_declared_version": "content-0",
    "plateforce_version": "content-0",
    "descriptions": {
        "jump_height_m": "0.3142 m\n  jumpheight.takeoff.velocity\n    takeoff.threshold.absolute_force",
        "takeoff_velocity_m_per_s": "2.4832 m/s\n  takeoff.threshold.absolute_force",
    },
}


def an_agreeing_value(field):
    """The value the surfaces carry for one field asserted another way, in that field's shape."""
    if field not in AN_AGREEING_VALUE:
        raise SystemExit(
            f"plateforce: {field} is asserted another way and no value of its shape is written "
            "here, so every case below would run against a field the assertion cannot read"
        )
    return copy.deepcopy(AN_AGREEING_VALUE[field])


def a_recorded_divergence(kind, request):
    """One field the register records as reaching some surfaces and agreeing across them.

    Read off the register rather than named, because a case naming one field goes quiet the
    moment that entry is discharged and this script is run by hand. `descriptions` was named
    here until it reached all four surfaces, which left the case writing a field the register
    no longer describes and refusing for a reason that is not this one.

    Agreeing rather than any entry at all: spreading a divergence whose carriers disagree
    raises the disagreement fault as well, and a case that trips two refusals proves neither.
    Filled by this request for the same reason: an entry the request does not fill is on no
    answer here, and spreading it would raise the entry's own refusal instead.
    """
    for field, declared in gate.SURFACES_THAT_DIFFER[kind].items():
        if declared.carriers_agree and fills(declared, request):
            return field
    raise SystemExit(
        f"plateforce: the {kind} register records no divergence its carriers agree about on "
        f"the {request} request, so there is nothing here to spread to every surface"
    )


def fills(declared, request):
    """Whether this request is one the entry says puts the field on a wire.

    An entry naming no request is one every request of its kind fills, which is what a field
    the request cannot decide looks like: the terminal names the build on every analysis it
    performs, and no request makes it stop.
    """
    return not declared.filled_by or request in declared.filled_by


def a_field_the_request_leaves_off_every_wire(kind, request):
    """One field the document declares that no answer to this request carries.

    Read off the register rather than named, for the reason `a_recorded_divergence` is: a case
    naming `plate_profile` goes quiet the moment that entry is discharged, and this script is
    run by hand.
    """
    for field, declared in gate.SURFACES_THAT_DIFFER[kind].items():
        if not fills(declared, request):
            return field
    raise SystemExit(
        f"plateforce: the {kind} register records no field a request decides, so on the "
        f"{request} request there is no declared field whose account can be taken away"
    )


def a_field_the_request_fills(kind, request):
    """One field on a wire because this request asked for it, and on no other request's."""
    for field, declared in gate.SURFACES_THAT_DIFFER[kind].items():
        if declared.filled_by and request in declared.filled_by:
            return field
    raise SystemExit(
        f"plateforce: the {kind} register records no field the {request} request puts on a "
        "wire, so there is nothing here to stop filling"
    )


def answers_from_baseline(baseline, kind, asked, request):
    """The surfaces as they answer today, assembled from a committed result.

    The fields beyond the compared ones are placed from the gate's own register rather than
    from a table here, so this file cannot drift from what the gate expects and then report the
    drift as a case that proved something. A field recorded as agreeing gets one value across
    its carriers, one recorded as disagreeing gets a different value on each, and the surfaces
    are exactly the ones that request is asked of.

    A divergence this request does not fill is left off, because that is what the surfaces do
    with it: `plate_profile` is on two answers to the request that states a plate and on
    nobody's answer to the rest, so placing it everywhere would build a document no surface
    produces and every case below would run against it.
    """
    document = json.loads(baseline.read_text(encoding="utf-8"))
    result = document["result"]
    answers = {surface: dict(result) for surface in asked}

    # Published by every surface and covered by an assertion of its own rather than by the
    # comparison. Each assertion reads the value, so the surfaces have to match, and each
    # reads it in that field's own shape.
    for field in gate.ASSERTED_ANOTHER_WAY[kind]:
        for answer in answers.values():
            answer[field] = an_agreeing_value(field)

    for field, declared in gate.SURFACES_THAT_DIFFER[kind].items():
        if not fills(declared, request):
            continue
        for surface in declared.carried_by:
            answers[surface][field] = (
                f"{surface}-{field}" if declared.carriers_agree is False else f"one-{field}"
            )
    return answers, document["compared_fields"]


def answers_that_pass():
    return answers_from_baseline(BASELINE, gate.ANALYSED, gate.surfaces_named_in_manifest(), QUIET)


def plate_answers_that_pass():
    """The request that states a saved plate, over the four surfaces it is asked of.

    A second analysed control, because the reach register is keyed by request as well as by
    kind: `plate_profile` is on two answers here and on nobody's answer to the request above,
    so a case written against one says nothing about the other.
    """
    asked = gate.surfaces_named_in_manifest()
    return answers_from_baseline(PLATE_BASELINE, gate.ANALYSED, asked, PLATE), asked


def swept_answers_that_pass():
    """The sweep, over the surfaces that can be asked one.

    A separate control because the register is per kind: an analysed document carries
    `plateforce_version` on two surfaces and a swept one on every surface that answers, so a
    case written against one says nothing about the other.
    """
    asked = gate.surfaces_named_in_manifest() - set(gate.SURFACES_NOT_ASKED[gate.SWEPT])
    return answers_from_baseline(SWEEP_BASELINE, gate.SWEPT, asked, SWEEP), asked


def faults_when(name, change, expected, kind=None, build=None, request=QUIET):
    """Apply one change to a passing run and require a fault that names `expected`.

    Every register a case can reach is restored afterwards, because several cases edit one: a
    recorded disagreement is only repairable by a case that first records one, and a register
    emptied or widened here would change what every case after it is measured against. Which
    struct a kind is answered in is restored for the same reason, and it is the loudest of the
    three: left pointing elsewhere it would redden every remaining case for a reason that is
    not the case.
    """
    kind = kind or gate.ANALYSED
    differ_was = dict(gate.SURFACES_THAT_DIFFER[kind])
    never_was = dict(gate.NEVER_ON_THE_WIRE[kind])
    document_was = gate.DOCUMENT_OF_KIND[kind]
    if build is None:
        answers, fields = answers_that_pass()
        asked = gate.surfaces_named_in_manifest()
    else:
        (answers, fields), asked = build()
    fields = list(fields)
    try:
        fields = change(answers, fields) or fields
        print(f"applied {name}", flush=True)
        faults = gate.coverage_faults(answers, fields, kind, asked, request)
    finally:
        gate.SURFACES_THAT_DIFFER[kind].clear()
        gate.SURFACES_THAT_DIFFER[kind].update(differ_was)
        gate.NEVER_ON_THE_WIRE[kind].clear()
        gate.NEVER_ON_THE_WIRE[kind].update(never_was)
        gate.DOCUMENT_OF_KIND[kind] = document_was
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
    faults = gate.coverage_faults(
        answers, fields, gate.ANALYSED, gate.surfaces_named_in_manifest(), QUIET
    )
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no case below means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(fields)} compared fields, four surfaces, no fault", flush=True)
    return True


def a_plate_control_that_must_pass():
    """The request that states a saved plate, unchanged, has to be green.

    A second analysed control because a field can be on the wire because the request asked for
    it, and a case written against the request that states no plate is measured against a
    document `plate_profile` is absent from.
    """
    (answers, fields), asked = plate_answers_that_pass()
    print("applied nothing, the plate control", flush=True)
    faults = gate.coverage_faults(answers, fields, gate.ANALYSED, asked, PLATE)
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no plate case means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(fields)} compared fields, {len(asked)} surfaces, no fault", flush=True)
    return True


def a_swept_control_that_must_pass():
    (answers, fields), asked = swept_answers_that_pass()
    print("applied nothing, the swept control", flush=True)
    faults = gate.coverage_faults(answers, fields, gate.SWEPT, asked, SWEEP)
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
    """A field the register records as reaching some surfaces, reaching all of them."""
    field = a_recorded_divergence(gate.ANALYSED, QUIET)
    for answer in answers.values():
        answer[field] = f"one-{field}"


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


def make_two_surfaces_describe_one_number_differently(answers, fields):
    """One number, two accounts of itself, which is the defect that field is asserted for.

    A committed copy of an account moves with every registry data edit, so the account is held
    to the other surfaces rather than to a record. This is what holding it there has to catch,
    and the quantity is named off the block so the case cannot outlive a renamed metric.
    """
    block = answers["r"]["descriptions"]
    quantity = sorted(block)[0]
    block[quantity] = block[quantity].replace("m", "cubits")


def empty_the_account_every_surface_gives(answers, fields):
    """Four surfaces describing nothing, which reads exactly like four surfaces agreeing.

    The hollow the assertion is floored against: an engine change that stops writing the block
    leaves every surface's copy of it empty, every pair of them identical, and a comparison
    that walks no quantity reporting no fault.
    """
    for answer in answers.values():
        answer["descriptions"] = {}


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


def stop_naming_a_field_no_answer_here_carries(answers, fields):
    """The defect the reach registers were added for, put back.

    A field the document declares that no answer to this request carries is invisible to every
    register keyed by what the answers hold: `serde` drops it, so it is compared by nobody,
    asserted by nobody and missing from nobody, and the gate goes on reporting that every field
    was accounted for. Taking away its entry is the state the gate was in before it read the
    document at all.

    Written against the register rather than against a named field, for the reason the repaired
    disagreement above is: a case naming `plate_profile` goes quiet the moment that entry
    moves, and this script is run by hand.
    """
    gate.SURFACES_THAT_DIFFER[gate.ANALYSED].pop(
        a_field_the_request_leaves_off_every_wire(gate.ANALYSED, QUIET)
    )


def name_a_field_the_document_does_not_declare(answers, fields):
    """A register entry for a field no document has, which reads as coverage and covers nothing."""
    gate.NEVER_ON_THE_WIRE[gate.ANALYSED]["calibration_certificate"] = gate.NeverOnTheWire(
        "nothing, because there is no such field", "a field this document never declared"
    )


def put_a_field_named_as_reaching_no_wire_on_every_surface(answers, fields):
    """The entry outliving the gap it records, which an allow-list would pass in silence.

    The entry is moved into the register rather than read out of it: both registers are empty
    today, so a case that iterated one would apply nothing and report whatever the run happened
    to raise. Put on every surface and compared as well, so the entry itself is the only thing
    left wrong with the run and the refusal that fires is this one.
    """
    field = a_field_the_request_leaves_off_every_wire(gate.ANALYSED, QUIET)
    gate.SURFACES_THAT_DIFFER[gate.ANALYSED].pop(field)
    gate.NEVER_ON_THE_WIRE[gate.ANALYSED][field] = gate.NeverOnTheWire(
        "a request that fills it", "a field every request in this population leaves off"
    )
    for answer in answers.values():
        answer[field] = {"name": "parity-lab-plate"}
    fields.append(field)
    return fields


def carry_a_field_this_request_does_not_fill(answers, fields):
    """A divergence recorded as one request's, on the answers to another.

    The register says which requests put a field on a wire, so an entry that has stopped being
    true of the population reddens here rather than excusing the field on every request at
    once.
    """
    field = a_field_the_request_leaves_off_every_wire(gate.ANALYSED, QUIET)
    for surface in gate.SURFACES_THAT_DIFFER[gate.ANALYSED][field].carried_by:
        answers[surface][field] = f"one-{field}"


def stop_filling_the_field_this_request_fills(answers, fields):
    """The one request that puts a field on a wire, stopping.

    The gap reopening, which is the direction the register cannot see without being asked:
    every other request leaves the field off every wire and the entry accounts for that, so
    without this the register would go on describing a divergence the population no longer has
    and the field would be back where `NEVER_ON_THE_WIRE` found it.
    """
    field = a_field_the_request_fills(gate.ANALYSED, PLATE)
    for answer in answers.values():
        answer.pop(field, None)


def make_the_document_a_stranger_to_the_surfaces(answers, fields):
    """The control on the parse itself: a universe the surfaces are not in.

    A read that found the wrong struct, or stopped early, reports a set of fields with nothing
    in it wrong on its face, and every field the surfaces publish then looks undeclared. That
    is what says the parse ran and reached the document rather than something shaped like it.
    """
    gate.DOCUMENT_OF_KIND[gate.ANALYSED] = "TrialSource"


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
        "two surfaces giving one number two accounts of itself",
        make_two_surfaces_describe_one_number_differently,
        "different accounts of itself",
    ),
    (
        "every surface's account of every number emptied at once",
        empty_the_account_every_surface_gives,
        "four empty blocks",
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
    (
        "a field the document declares that reaches no wire and is named nowhere",
        stop_naming_a_field_no_answer_here_carries,
        "and no answer to this request carries it",
    ),
    (
        "a field named as reaching no wire that the document does not declare",
        name_a_field_the_document_does_not_declare,
        "declares no such field",
    ),
    (
        "a field named as reaching no wire, on a wire",
        put_a_field_named_as_reaching_no_wire_on_every_surface,
        "it is on the wire",
    ),
    (
        "a field recorded as one request's, carried by another request's answers",
        carry_a_field_this_request_does_not_fill,
        "is recorded as on the wire on",
    ),
    (
        "the document's fields read off something that is not the document",
        make_the_document_a_stranger_to_the_surfaces,
        "declares no such field, so the",
    ),
]


# The request that states a saved plate, whose answers carry a field the four above them leave
# off every wire. Every case above is measured against a request that states no plate, so this
# is the only place the entry's own account of which requests fill it can be broken.
PLATE_CASES = [
    (
        "the one request that puts a field on a wire, leaving it off",
        stop_filling_the_field_this_request_fills,
        "which it is recorded as reaching",
    ),
    (
        "a field one surface publishes on the request that states a plate, and the others do not",
        add_a_field_to_one_surface,
        "sample_rate_hz_read",
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


def every_request_the_register_rests_on():
    """Every request a reach entry says puts a field on a wire, derived rather than written out.

    An entry naming a request the population does not hold is a refusal, and a control that
    tripped it would say nothing about any case below. Written here as a list it would go stale
    the moment an entry named a different request.
    """
    return sorted(
        {
            name
            for register in gate.SURFACES_THAT_DIFFER.values()
            for declared in register.values()
            for name in declared.filled_by
        }
    )


def a_row_for(name):
    """One row of a passing population, in the naming every committed request already uses."""
    return (
        f"{name}\ttests/golden/result-parity-request-{name}.json\t"
        f"tests/golden/result-parity-{name}.json\t{every_surface()}"
    )


# A row for each thing the registers rest on, because two entries hold an account of themselves
# that a population can falsify. One rests on a swept request being asked at all; the others
# name the requests that put a field on a wire. A population missing either makes that entry
# refuse, which is what it is there to do.
A_POPULATION_THAT_PASSES = [
    f"quiet\ttests/golden/result-parity-request.json\ttests/golden/result-parity.json\t{every_surface()}",
    f"sentinel\ttests/golden/result-parity-request-sentinel.json\t=quiet\t{every_surface()}",
    *(a_row_for(name) for name in every_request_the_register_rests_on()),
    f"sweep\ttests/golden/result-parity-request-sweep.json\ttests/golden/result-parity-sweep.json\t{every_surface_a_sweep_reaches()}",
]

DEFAULT_REQUESTS = [
    "result-parity-request.json",
    "result-parity-request-sentinel.json",
    *(f"result-parity-request-{name}.json" for name in every_request_the_register_rests_on()),
    "result-parity-request-sweep.json",
]
DEFAULT_BASELINES = [
    "result-parity.json",
    *(f"result-parity-{name}.json" for name in every_request_the_register_rests_on()),
    "result-parity-sweep.json",
]


def row_named(name):
    """The passing population's row for one request, found by name rather than by position.

    A row added to the population shifts every index after it, and a case that reached for one
    by number would quietly start changing a different row and refuse for a reason that is not
    the case.
    """
    return next(row for row in A_POPULATION_THAT_PASSES if row.startswith(f"{name}\t"))


def population_where(name, row):
    """The passing population with one row replaced, so a case changes one thing."""
    return [row if held.startswith(f"{name}\t") else held for held in A_POPULATION_THAT_PASSES]


def population_without(name):
    """The passing population with one row taken out."""
    return [held for held in A_POPULATION_THAT_PASSES if not held.startswith(f"{name}\t")]


def files_without(names, held):
    """Every file of a population but the ones belonging to the named requests."""
    return [file for file in held if not any(f"-{name}." in file for name in names)]


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
ANALYSED_ROWS = population_without(SWEEP)
ANALYSED_REQUESTS = files_without([SWEEP], DEFAULT_REQUESTS)
ANALYSED_BASELINES = files_without([SWEEP], DEFAULT_BASELINES)

# One request a reach entry rests on, for the case that takes it out of the population. Read
# off the register rather than named, and empty where no entry names a request at all, which
# is the state that leaves the case with nothing to remove.
RESTED_ON = every_request_the_register_rests_on()

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
            rows=population_where("sentinel", row_named("sentinel").replace("=quiet", "=loud")),
            expected="and no row is named that",
        ),
    ),
    (
        "a row declared equal to itself, which asserts nothing",
        dict(
            rows=population_where(
                "sentinel", row_named("sentinel").replace("=quiet", "=sentinel")
            ),
            expected="declared equal to itself",
        ),
    ),
    (
        "a chain of rows each declared equal to the next",
        dict(
            rows=population_where(
                QUIET, row_named(QUIET).replace("tests/golden/result-parity.json", "=sentinel")
            ),
            expected="One hop",
        ),
    ),
    (
        "two rows carrying one name",
        dict(
            rows=[row_named(QUIET)] + A_POPULATION_THAT_PASSES,
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
            rows=population_where(QUIET, f"{row_named(QUIET)},abacus"),
            expected="names no such surface",
        ),
    ),
    (
        # A population of one agrees with itself, which is the shape this project keeps
        # catching one level down, in a divergence carried by a single surface.
        "a request one surface answers",
        dict(
            rows=population_where(QUIET, row_named(QUIET).rsplit("\t", 1)[0] + "\tcli"),
            expected="agreeing with itself",
        ),
    ),
    (
        "a surface left off a request and named nowhere",
        dict(
            rows=population_where(
                QUIET,
                row_named(QUIET).replace(every_surface(), every_surface_a_sweep_reaches()),
            ),
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
            rows=population_where(
                SWEEP,
                row_named(SWEEP).replace(every_surface_a_sweep_reaches(), every_surface()),
            ),
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
            requests=ANALYSED_REQUESTS,
            baselines=ANALYSED_BASELINES,
            expected="reads as coverage and covers nothing",
        ),
    ),
    (
        "a register entry resting on a swept request the population no longer holds",
        dict(
            rows=ANALYSED_ROWS,
            requests=ANALYSED_REQUESTS,
            baselines=ANALYSED_BASELINES,
            expected="its account of itself is a sentence nothing measures",
        ),
    ),
] + [
    (
        # The register says which requests put a field on a wire. A population without one of
        # them leaves the entry excusing that field's absence everywhere and nothing ever
        # putting it anywhere, which reads as coverage and covers nothing.
        f"a reach entry resting on the {name} request, which the population no longer holds",
        dict(
            rows=population_without(name),
            requests=files_without([name], DEFAULT_REQUESTS),
            baselines=files_without([name], DEFAULT_BASELINES),
            expected="so nothing ever puts it on one",
        ),
    )
    for name in RESTED_ON
]


# The record of the values no projection can hold. A field two surfaces carry cannot go in
# `result`, which every surface is held to whole, so it is held in `carried_by_some` instead,
# and every refusal that block rests on is a case here. Without them the block is a second
# standard nothing is measured against, which is the defect one level up from a baseline no
# row is held to.

def a_divergence_held_to_the_record(kind, answers):
    """One recorded divergence these answers put on a wire, read off the register.

    Named nowhere, for the reason `a_recorded_divergence` is named nowhere: a case naming
    `plate_profile` goes quiet the moment that entry names something that would move a record,
    and this script is run by hand.
    """
    for field in sorted(gate.divergences_held_to_a_record(kind)):
        if gate.surfaces_publishing(answers, field):
            return field
    raise SystemExit(
        f"plateforce: no {kind} divergence is both held to a record and on a wire here, so "
        "there is no record for a case to break"
    )


def a_record_that_matches():
    """A committed block of divergent values, and the answers it was taken from.

    Taken from the answers by the gate's own writer rather than assembled here, so a case is a
    change against what a regeneration would commit rather than against a document written in
    this file.
    """
    (answers, _), _ = plate_answers_that_pass()
    return gate.divergent_values_measured_from(answers, gate.ANALYSED), answers


def record_faults_when(name, change, expected):
    """Apply one change to a matching record and require a fault that names `expected`.

    No register is restored afterwards, because no case here edits one: each changes the
    committed block or one carrier's answer, and both are built fresh for every case.
    """
    committed, answers = a_record_that_matches()
    change(committed, answers)
    print(f"applied {name}", flush=True)
    faults = gate.divergent_record_faults(committed, answers, gate.ANALYSED)
    hit = [fault for fault in faults if expected in fault]
    if not hit:
        print(f"  NOT REFUSED: no fault mentions {expected!r}", file=sys.stderr)
        for fault in faults:
            print(f"    the faults raised were: {fault}", file=sys.stderr)
        return False
    print(f"  refused: {hit[0].splitlines()[0][:150]}", flush=True)
    return True


def a_record_control_that_must_pass():
    committed, answers = a_record_that_matches()
    print("applied nothing, the record control", flush=True)
    faults = gate.divergent_record_faults(committed, answers, gate.ANALYSED)
    if faults:
        print("  THE CONTROL DOES NOT PASS, so no record case means anything:", file=sys.stderr)
        for fault in faults:
            print(f"    {fault}", file=sys.stderr)
        return False
    print(f"  passed: {len(committed)} divergent values held to the record, no fault")
    return True


def take_a_value_out_of_the_record(committed, answers):
    """A field on two wires and nothing committed for it, which is where every one of these
    fields sat until the block existed."""
    committed.pop(a_divergence_held_to_the_record(gate.ANALYSED, answers))


def move_a_carrier_away_from_the_record(committed, answers):
    """One carrier reporting something the record does not say.

    The claim a record makes that carriers-agree cannot: two surfaces wrong the same way agree
    perfectly, and only a committed value puts the change in front of a reviewer.
    """
    field = a_divergence_held_to_the_record(gate.ANALYSED, answers)
    carrier = sorted(gate.surfaces_publishing(answers, field))[0]
    answers[carrier][field] = "went-its-own-way"


def record_a_value_the_register_does_not_record(committed, answers):
    """A value in the block for a field nothing holds to it, which is a standard nothing is
    measured against."""
    committed["registry_digest"] = "content-0"


RECORD_CASES = [
    (
        "a divergence on a wire that the record holds no value for",
        take_a_value_out_of_the_record,
        "the record holds no value for it",
    ),
    (
        "a carrier reporting something the record does not say",
        move_a_carrier_away_from_the_record,
        "does not match the record of",
    ),
    (
        "a value in the record for a field the register does not record",
        record_a_value_the_register_does_not_record,
        "the register does not record that field",
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
    if not a_plate_control_that_must_pass():
        raise SystemExit(1)
    survived += [
        name
        for name, change, expected in PLATE_CASES
        if not faults_when(
            name, change, expected, build=plate_answers_that_pass, request=PLATE
        )
    ]

    print()
    if not a_swept_control_that_must_pass():
        raise SystemExit(1)
    survived += [
        name
        for name, change, expected in SWEPT_CASES
        if not faults_when(
            name, change, expected, kind=gate.SWEPT, build=swept_answers_that_pass, request=SWEEP
        )
    ]

    print()
    if not a_manifest_control_that_must_pass():
        raise SystemExit(1)
    for name, case in MANIFEST_CASES:
        if not manifest_faults_when(name, **case):
            survived.append(name)

    print()
    if not a_record_control_that_must_pass():
        raise SystemExit(1)
    for name, change, expected in RECORD_CASES:
        if not record_faults_when(name, change, expected):
            survived.append(name)

    print()
    if not a_hollow_control_that_must_pass():
        raise SystemExit(1)
    for name, build, expected in HOLLOW_CASES:
        committed, fields = build()
        if not hollow_faults_when(name, committed, fields, expected):
            survived.append(name)

    total = (
        len(CASES)
        + len(PLATE_CASES)
        + len(SWEPT_CASES)
        + len(MANIFEST_CASES)
        + len(RECORD_CASES)
        + len(HOLLOW_CASES)
    )
    print()
    print(f"{total - len(survived)} of {total} cases were refused")
    if survived:
        for name in survived:
            print(f"plateforce: {name} did not make the gate refuse", file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
