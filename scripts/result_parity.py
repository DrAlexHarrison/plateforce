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
"""

import json
import sys

# A floor rather than the exact count, which would fail every time a metric is added. It is
# here because a comparison that agreed on an empty document would report success.
NUMBERS_A_RESULT_CARRIES = 40


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


def write(baseline_path, answers):
    """One surface writes the baseline and every surface is then held to it.

    Regenerating from whichever surface answered first would record a defect as the standard
    if that surface had one, so the diff is audited by hand, the discipline
    `crates/plateforce-analysis/tests/resolved-values-baseline.txt` already carries.
    """
    fields = compared_fields_in(baseline_path)
    first = sorted(answers)[0]
    document = {
        "compared_fields": fields,
        "result": projected(answers[first], fields, first),
    }
    with open(baseline_path, "w", encoding="utf-8") as handle:
        handle.write(json.dumps(document, sort_keys=True, indent=2) + "\n")
    print(f"{baseline_path} written from {first}; audit the diff before committing it")


def check(baseline_path, answers):
    fields = compared_fields_in(baseline_path)
    with open(baseline_path, encoding="utf-8") as handle:
        committed = json.load(handle)["result"]

    faults = []

    # Asserted between the surfaces rather than against a committed value. The digest moves
    # with every registry edit, so freezing one would redden this check for a reason that is
    # not about parity, while two surfaces reading different registries is exactly parity.
    digests = {name: answer.get("registry_digest") for name, answer in answers.items()}
    if len(set(digests.values())) > 1:
        faults.append(f"surfaces read different registries: {digests}")

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
    print(
        f"{len(answers)} of {len(answers)} surfaces computed the committed result, "
        f"{values} numbers each"
    )


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
