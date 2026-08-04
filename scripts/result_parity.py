#!/usr/bin/env python3
"""Compare one result, computed on every surface, against one committed document.

Against a committed baseline rather than between the surfaces: several surfaces wrong the
same way agree with each other perfectly, and a committed file makes every change a diff a
reviewer reads.

What this does not prove, stated because a green here is easy to over-read. `serde_json`
writes the shortest string that round-trips, so the wire text is identical whether or not a
build carries `float_roundtrip`, and only the double a caller ends up holding differs. Bytes
therefore cannot see that defect at all. The reading side is asserted inside each runtime
against a second reader in that same runtime, because five surfaces sharing one wrong parser
agree with each other: `crates/plateforce-cli/tests/result_parity.rs` for Rust and
`bindings/r/tests/testthat/test-doubles.R` for R.

How far the comparison reaches is measured from the surfaces rather than listed. Every field
every surface publishes is compared here, asserted another way in `ASSERTED_ANOTHER_WAY`, or
carried by some surfaces and not others and recorded in `SURFACES_THAT_DIFFER`. A field in
none of the three makes this gate refuse, so it cannot print that four surfaces computed the
result while a field of that result went unread. `scripts/prove-parity-coverage-refuses.py`
is where each of those refusals is shown to fire.
"""

import json
import pathlib
import sys

# A floor rather than the exact count, which would fail every time a metric is added. It is
# here because a comparison that agreed on an empty document would report success.
#
# Measured rather than picked, by the count this module prints: the compared fields carry 22
# numbers on the committed request, 11 metric values, 5 levels, 5 landmark indices and the
# gravity on `bound_globals`. `bound_methods` contributes none, because a bound parameter
# travels as the text beside its name. The floor sits below that so a metric coming or going
# is not an alarm, and far enough above zero to catch a projection that collapsed.
NUMBERS_A_RESULT_CARRIES = 10

# Every surface the gate speaks for, read from the manifest the harness reads.
SURFACE_MANIFEST = pathlib.Path(__file__).with_name("result-parity-surfaces.txt")


def canonical(value):
    """One writer for every side of every comparison, applied to each surface's own bytes.

    A surface cannot pass by formatting differently in a way the comparison forgives, and a
    surface cannot fail for putting a space somewhere.
    """
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def result_in(envelope, name):
    if not isinstance(envelope, dict) or "ok" not in envelope:
        raise SystemExit(f"plateforce: {name} returned no result: {canonical(envelope)[:400]}")
    return envelope["ok"]


def answers_from(arguments):
    answers = {}
    for argument in arguments:
        name, _, path = argument.partition("=")
        with open(path, encoding="utf-8") as handle:
            answers[name] = result_in(json.load(handle), name)
    return answers


def differing_paths(committed, reported, path=""):
    """Name the field rather than a byte offset, so a failure says what moved."""
    if isinstance(committed, dict) and isinstance(reported, dict):
        found = []
        for key in sorted(set(committed) | set(reported)):
            here = f"{path}.{key}" if path else key
            if key not in committed:
                found.append(f"{here}: only the surface has it")
            elif key not in reported:
                found.append(f"{here}: only the committed result has it")
            else:
                found += differing_paths(committed[key], reported[key], here)
        return found
    if isinstance(committed, list) and isinstance(reported, list):
        if len(committed) != len(reported):
            return [f"{path}: committed holds {len(committed)}, the surface holds {len(reported)}"]
        found = []
        for index, (was, now) in enumerate(zip(committed, reported)):
            found += differing_paths(was, now, f"{path}[{index}]")
        return found
    if committed != reported:
        return [f"{path}: committed {committed!r}, the surface reports {reported!r}"]
    return []


def projected(answer, fields, name):
    """The engine's own response fields, which every surface carries.

    Each surface wraps them in an envelope of its own and adds what only it knows, a file
    path here, a description block there. Comparing whole envelopes would report those as
    divergence and say nothing about the numbers, so the comparison is over the fields the
    engine produced and a surface that stops carrying one fails by name.
    """
    missing = [field for field in fields if field not in answer]
    if missing:
        raise SystemExit(f"plateforce: {name} carries no {missing}, which every surface reports")
    return {field: answer[field] for field in fields}


def compared_fields_in(baseline_path):
    try:
        with open(baseline_path, encoding="utf-8") as handle:
            fields = json.load(handle)["compared_fields"]
    except (OSError, KeyError, json.JSONDecodeError):
        raise SystemExit(
            f"plateforce: {baseline_path} names no compared_fields, and which fields every "
            "surface owes is a decision rather than a measurement"
        )
    if not fields:
        raise SystemExit("plateforce: compared_fields is empty, so this gate asserts nothing")
    return fields


def surfaces_named_in_manifest():
    """Every surface this gate speaks for, read from the file the harness reads.

    A run over some of the surfaces would report agreement between the ones it asked and read
    as agreement between the four the manifest names, and would widen the intersection below
    at the same time. This gate speaks for every listed surface or it does not speak.
    """
    named = []
    for line in SURFACE_MANIFEST.read_text(encoding="utf-8").splitlines():
        row = line.strip()
        if row and not row.startswith("#"):
            named.append(row.split("\t")[0].strip())
    if not named:
        raise SystemExit(f"plateforce: {SURFACE_MANIFEST} names no surface")
    return set(named)


def fields_every_surface_publishes(answers):
    """The whole of what this comparison can reach, measured rather than declared.

    `projected` raises on a field a surface does not carry, so a field one surface publishes
    and another does not cannot be held to a single committed value at all. This set is
    therefore not a judgement about which fields are worth comparing. It is the reach of the
    mechanism, and every field in it owes either a comparison or an assertion of its own.
    """
    return set.intersection(*(set(answer) for answer in answers.values()))


def surfaces_publishing(answers, field):
    return frozenset(name for name, answer in answers.items() if field in answer)


# A field every surface publishes that this gate does not compare against the committed
# document, with the assertion that covers it instead. Naming a field here is not permission
# to stop looking at it: each entry owes a function, `coverage_faults` runs it, and an entry
# without one cannot be written. Widening `compared_fields` is the ordinary answer and this
# is the exception, so an entry states why a committed value would be the wrong instrument.
ASSERTED_ANOTHER_WAY = {
    "registry_digest": (
        "the digest moves with every registry edit, so a committed value would redden this "
        "gate for a reason that is not about parity, while two surfaces reading different "
        "registries is exactly parity",
        lambda answers: surfaces_read_one_registry(answers),
    ),
}

# A field some surfaces publish and others do not, with why they differ. This gate cannot
# compare one, because there is no value on every surface to compare, so the register exists
# to make the divergence something a reader of this gate's output can see rather than
# something its silence hides.
#
# Checked against measurement in both directions on every run. A field that reached a fifth
# surface, or stopped reaching one it is declared to reach, has changed what a result carries
# on some surface, and a line here still naming the old set is not a reason to pass over it.
# An entry is a divergence recorded, never a divergence accepted.
SURFACES_THAT_DIFFER = {
    "plateforce_version": (
        frozenset({"cli", "browser"}),
        "the build that produced the numbers, carried by the two surfaces that assemble "
        "`ResultDocument`. Python and R answer for a build a caller already holds, through "
        "`plateforce.__version__` and `packageVersion`",
    ),
    "registry_version": (
        frozenset({"cli", "browser", "python"}),
        "the version the registry declares. R alone omits it, which is an omission rather "
        "than a decision: `bindings/r/src/rust/src/lib.rs` builds its own report and has no "
        "field for it",
    ),
    "trial": (
        frozenset({"cli", "browser"}),
        "where the trace came from and what the reader had to be told about reading it. The "
        "two surfaces handed a path know it; Python and R are handed a trial somebody else "
        "opened",
    ),
    "descriptions": (
        frozenset({"r"}),
        "the account each quantity gives of itself. Generated in `descriptions_of`, which "
        "lives in R's binding and in no other surface's, so a terminal, a browser tab and a "
        "notebook receive nothing here",
    ),
    "spread": (
        frozenset({"cli"}),
        "how far a number moves across a slot's defensible alternatives. The terminal sweeps "
        "with the analysis; the tab sweeps on its own schedule through a second entry point, "
        "and Python and R expose the sweep as a call of its own",
    ),
}


def surfaces_read_one_registry(answers):
    """Asserted between the surfaces rather than against a committed value.

    Two surfaces resolving rules out of different registries agree about nothing that matters,
    however well their numbers happen to line up.
    """
    digests = {name: answer.get("registry_digest") for name, answer in answers.items()}
    if len(set(digests.values())) > 1:
        return [f"surfaces read different registries: {digests}"]
    return []


def compared_fields_measured_from(answers):
    """What `compared_fields` should name: everything the surfaces all publish, less the
    fields asserted another way.

    Derived rather than maintained, which is the whole point. Read out of the baseline it
    checks, this list could only ever be widened by hand, so a field added to all four
    surfaces stayed invisible to the gate until somebody noticed.
    """
    return sorted(fields_every_surface_publishes(answers) - set(ASSERTED_ANOTHER_WAY))


def coverage_faults(answers, fields):
    """Every field a surface publishes is compared here, asserted another way, or a declared
    divergence. A field in none of the three is a field nothing looks at.

    This gate prints that four surfaces computed one result. A reader takes that to be about
    the result, not about the six fields of it somebody listed, so the gate refuses rather
    than publish a verdict narrower than its own sentence.
    """
    faults = []
    named = surfaces_named_in_manifest()
    if set(answers) != named:
        faults.append(
            f"the manifest names {sorted(named)} and this run holds {sorted(answers)}, so "
            "nothing below speaks for the surfaces this gate claims"
        )
        return faults

    everywhere = fields_every_surface_publishes(answers)
    somewhere = set.union(*(set(answer) for answer in answers.values()))

    unread = sorted(everywhere - set(fields) - set(ASSERTED_ANOTHER_WAY))
    if unread:
        faults.append(
            f"every surface publishes {unread} and nothing here looks at any of them, so "
            f"this gate would report agreement about {len(fields)} of "
            f"{len(everywhere)} fields and say four surfaces computed the result. Regenerate "
            "with scripts/result-parity.sh --write, which derives the list from the surfaces"
        )

    for field in sorted(set(fields) - everywhere):
        faults.append(
            f"compared_fields names {field} and only {sorted(surfaces_publishing(answers, field))} "
            f"publish it, so it cannot be held to one committed value"
        )

    for field in sorted(set(fields) & set(ASSERTED_ANOTHER_WAY)):
        faults.append(
            f"{field} is compared against the committed document and also declared asserted "
            "another way, so one of the two is wrong"
        )

    for field, (reason, assertion) in sorted(ASSERTED_ANOTHER_WAY.items()):
        if field not in somewhere:
            faults.append(
                f"{field} is declared asserted another way and no surface publishes it, which "
                f"reads as coverage and covers nothing: {reason}"
            )
            continue
        faults += assertion(answers)

    for field in sorted(somewhere - everywhere):
        carried = surfaces_publishing(answers, field)
        declared = SURFACES_THAT_DIFFER.get(field)
        if declared is None:
            faults.append(
                f"{field} reaches {sorted(carried)} and not {sorted(set(answers) - carried)}, "
                "and nothing here says why. A field one surface publishes and another drops is "
                "a result carrying its method on some surfaces and not others"
            )
        elif declared[0] != carried:
            faults.append(
                f"{field} is declared to reach {sorted(declared[0])} and reaches "
                f"{sorted(carried)}: {declared[1]}"
            )

    for field, (declared, reason) in sorted(SURFACES_THAT_DIFFER.items()):
        if field in everywhere:
            faults.append(
                f"{field} now reaches every surface, so it is comparable and belongs in "
                f"compared_fields rather than here: {reason}"
            )
        elif field not in somewhere:
            faults.append(
                f"{field} is declared uneven across the surfaces and no surface publishes it: "
                f"{reason}"
            )

    return faults


def write(baseline_path, answers):
    """One surface writes the baseline and every surface is then held to it.

    Regenerating from whichever surface answered first would record a defect as the standard
    if that surface had one, so the diff is audited by hand, the discipline
    `crates/plateforce-analysis/tests/resolved-values-baseline.txt` already carries.
    """
    was = compared_fields_in(baseline_path)
    fields = compared_fields_measured_from(answers)

    # Regeneration widens and never narrows. A surface that stopped publishing a field would
    # otherwise take the field out of the list on the way past, and the gate would go on
    # reporting that every surface agreed, about less.
    dropped = sorted(set(was) - set(fields))
    if dropped:
        raise SystemExit(
            f"plateforce: {dropped} is compared today and some surface no longer publishes it, "
            "so regenerating would narrow this gate. Fix the surface, or take the name out of "
            "compared_fields by hand, which is the one direction that wants a decision"
        )

    faults = coverage_faults(answers, fields)
    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(
            "plateforce: the surfaces do not carry the shape this gate can compare, so a "
            "baseline written now would record that as the standard"
        )

    first = sorted(answers)[0]
    document = {
        "compared_fields": fields,
        "result": projected(answers[first], fields, first),
    }
    with open(baseline_path, "w", encoding="utf-8") as handle:
        handle.write(json.dumps(document, sort_keys=True, indent=2) + "\n")
    gained = sorted(set(fields) - set(was))
    print(f"{baseline_path} written from {first}; audit the diff before committing it")
    print(
        f"compared_fields derived from the surfaces: {len(fields)} fields"
        + (f", {len(gained)} newly covered: {gained}" if gained else ", none newly covered")
    )


def check(baseline_path, answers):
    fields = compared_fields_in(baseline_path)
    with open(baseline_path, encoding="utf-8") as handle:
        committed = json.load(handle)["result"]

    faults = coverage_faults(answers, fields)

    for name in sorted(answers):
        reported = projected(answers[name], fields, name)
        if canonical(reported) != canonical(committed):
            moved = differing_paths(committed, reported)
            where = "place" if len(moved) == 1 else "places"
            faults.append(
                f"{name} does not match the committed result in {len(moved)} {where}:\n    "
                + "\n    ".join(moved[:12])
                + ("\n    ..." if len(moved) > 12 else "")
            )

    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(1)

    values = sum(1 for _ in every_number(committed))
    if values < NUMBERS_A_RESULT_CARRIES:
        raise SystemExit(
            f"plateforce: the committed result holds {values} numbers, so a surface matching "
            f"it has agreed about almost nothing"
        )
    everywhere = fields_every_surface_publishes(answers)
    uneven = sorted(set.union(*(set(answer) for answer in answers.values())) - everywhere)
    print(
        f"{len(answers)} of {len(answers)} surfaces computed the committed result, "
        f"{values} numbers each"
    )
    # The denominator of the sentence above, so it cannot be read as a claim about the whole
    # document. Every field is accounted for: compared here, asserted another way, or a
    # divergence the surfaces carry and this comparison cannot reach.
    print(
        f"  {len(fields)} of {len(everywhere)} fields every surface publishes were compared, "
        f"{len(ASSERTED_ANOTHER_WAY)} asserted another way: {sorted(ASSERTED_ANOTHER_WAY)}"
    )
    for field in uneven:
        print(f"  {field} reaches {sorted(surfaces_publishing(answers, field))} of the four")


def every_number(value):
    """A control: a comparison over a document holding no numbers has proven nothing."""
    if isinstance(value, dict):
        for held in value.values():
            yield from every_number(held)
    elif isinstance(value, list):
        for held in value:
            yield from every_number(held)
    elif isinstance(value, (int, float)) and not isinstance(value, bool):
        yield value


def main():
    if len(sys.argv) < 3:
        raise SystemExit("usage: result_parity.py check|write <baseline> <name>=<file>...")
    mode, baseline_path, arguments = sys.argv[1], sys.argv[2], sys.argv[3:]
    answers = answers_from(arguments)
    if not answers:
        raise SystemExit("plateforce: no surface answered, so nothing was compared")
    if mode == "write":
        write(baseline_path, answers)
    elif mode == "check":
        check(baseline_path, answers)
    else:
        raise SystemExit(f"plateforce: {mode} is not check or write")


if __name__ == "__main__":
    main()
