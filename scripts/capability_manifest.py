#!/usr/bin/env python3
"""Hold every surface's answer about itself, and assert what has to be true across them.

Which entry points a surface dispatches, and which containers it can write a result into,
are facts about that surface. A comparison demanding one identical document from all of them
is satisfied only by a surface claiming something it cannot do, so each answer is recorded
under its own name and the obligations are asserted over the set instead.

Five assertions, and each fails for a different reason a reader can act on. A surface's own
answer must match what is committed for it, in both directions, so a lost entry point and an
undeclared one are both a diff. The engine's own tables must read the same everywhere, which
is what a stale build breaks. A fact about the engine riding inside a per-surface answer must
read the same everywhere for the same reason, which is the acquisition block's members: a
surface free to shorten that list would send a reader to find four of five and fingerprint an
incomplete block as matching. Every surface must reach every operation named in
`required_operations`. And that array must itself name operations some surface reaches, so a
misspelling is reported as a malformed obligation rather than as every surface failing.
"""

import json
import sys

# Facts about the engine every surface links, rather than about the surface that answered.
SHARED_KEYS = ("methods", "plateforce_version", "refusal_codes", "schema")

PER_SURFACE_KEYS = ("operations", "output_formats")

# A per-surface answer holding a fact about the engine inside it, named as the outer field
# and the member of it that cannot differ between surfaces. Whether a caller can state the
# acquisition block is a fact about the surface; which members the block holds is not.
SHARED_WITHIN = {"acquisition": "members"}

# The per-surface half of each of those, reported in the summary so a green run says how many
# surfaces take the block rather than only that they agree about it.
STATED_WITHIN = {"acquisition": "stated_by_caller"}

# Two, because that is the smallest number of surfaces a disagreement can exist between.
WITNESSES_A_COMPARISON_NEEDS = 2


def canonical(value):
    """One writer for both sides of every comparison, so a difference is about capability."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def capability_in(envelope, name):
    """A surface that declined to answer is a distinct failure from one that answered wrong."""
    if not isinstance(envelope, dict) or "ok" not in envelope:
        raise SystemExit(f"plateforce: {name} did not report a capability: {canonical(envelope)}")
    return envelope["ok"]


def answers_from(arguments):
    answers = {}
    for argument in arguments:
        name, _, path = argument.partition("=")
        with open(path, encoding="utf-8") as handle:
            answers[name] = capability_in(json.load(handle), name)
    return answers


def load(path):
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def write(manifest_path, answers):
    """`required_operations` is a ruling rather than a measurement, so it is carried forward.

    Regenerating it from the surfaces would make it the union of whatever they happen to do,
    which no surface could then fail.
    """
    try:
        required = load(manifest_path)["required_operations"]
    except (OSError, KeyError, json.JSONDecodeError):
        raise SystemExit(
            f"plateforce: {manifest_path} carries no required_operations to carry forward, "
            "and which operations every surface owes is a decision rather than a measurement"
        )
    document = {"required_operations": required, "surfaces": answers}
    with open(manifest_path, "w", encoding="utf-8") as handle:
        handle.write(json.dumps(document, sort_keys=True, indent=2) + "\n")
    print(f"{len(answers)} surfaces recorded in {manifest_path}")


def check(manifest_path, answers):
    manifest = load(manifest_path)
    required = manifest.get("required_operations")
    committed = manifest.get("surfaces", {})
    faults = []

    if not required:
        faults.append("required_operations is empty, so this gate asserts nothing")
    else:
        reached = {operation for answer in answers.values() for operation in answer["operations"]}
        for operation in required:
            if operation not in reached:
                faults.append(
                    f"required_operations names {operation}, which no surface reports at all"
                )

    for name in sorted(set(committed) ^ set(answers)):
        where = "committed but not asked" if name in committed else "asked but not committed"
        faults.append(f"{name} is {where}")

    for name in sorted(set(committed) & set(answers)):
        if canonical(answers[name]) != canonical(committed[name]):
            faults.append(
                f"{name} does not report what is committed for it:\n"
                + differences(committed[name], answers[name])
            )

    # A comparison's denominator is its witnesses. With one surface every assertion below
    # holds vacuously, because a set of one value has no second value to differ from and the
    # summary line reads exactly like a full pass. Dropping a row from the surfaces file is a
    # one-line edit, so the floor is asserted rather than assumed.
    if len(answers) < WITNESSES_A_COMPARISON_NEEDS:
        faults.append(
            f"{len(answers)} surface answered, and a claim about surfaces agreeing needs "
            f"{WITNESSES_A_COMPARISON_NEEDS}: one surface agrees with itself whatever it says"
        )

    for key in SHARED_KEYS:
        # Presence before agreement. `dict.get` returns None on both sides of a key nobody
        # carries, the two agree, and the check passes having compared two absences.
        absent = sorted(name for name, answer in answers.items() if key not in answer)
        if absent:
            faults.append(f"{absent} report no {key} at all, which every surface links")
            continue
        spellings = {name: canonical(answer[key]) for name, answer in answers.items()}
        if len(set(spellings.values())) > 1:
            grouped = sorted({value: name for name, value in spellings.items()}.values())
            faults.append(f"surfaces disagree on {key}, which every one of them links: {grouped}")

    for outer, inner in SHARED_WITHIN.items():
        # Presence at both depths before agreement, for the reason above: two absences
        # compare equal, and a check that compared them would pass having read nothing.
        absent = sorted(name for name, answer in answers.items() if outer not in answer)
        if absent:
            faults.append(f"{absent} report no {outer} at all, which every surface links")
            continue
        unnamed = sorted(
            name
            for name, answer in answers.items()
            if not isinstance(answer[outer], dict) or inner not in answer[outer]
        )
        if unnamed:
            faults.append(
                f"{unnamed} report {outer} without naming its {inner}, so a reader is told "
                f"the block exists and not what to go and find"
            )
            continue
        spellings = {name: canonical(answer[outer][inner]) for name, answer in answers.items()}
        if len(set(spellings.values())) > 1:
            grouped = sorted({value: name for name, value in spellings.items()}.values())
            faults.append(
                f"surfaces disagree on the {inner} of {outer}, which comes from the engine "
                f"every one of them links: {grouped}"
            )

    for name in sorted(answers):
        missing = [operation for operation in required or [] if operation not in answers[name]["operations"]]
        if missing:
            faults.append(f"{name} reaches {len(answers[name]['operations'])} operations and owes {missing}")

    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        raise SystemExit(1)

    print(
        f"{len(answers)} of {len(committed)} surfaces reported and matched "
        f"{manifest_path}, each owing {len(required)} operations, "
        f"agreeing across {len(answers)} surfaces on {len(SHARED_KEYS)} shared facts"
    )
    for outer, inner in SHARED_WITHIN.items():
        taking = sorted(
            name for name, answer in answers.items() if answer[outer].get(STATED_WITHIN[outer])
        )
        # Read off any surface, because the assertion above is what makes them one answer.
        named = next(iter(answers.values()))[outer][inner]
        print(
            f"{len(taking)} of {len(answers)} surfaces take the {outer} block, "
            f"and all {len(answers)} name the same {len(named)} {inner}: "
            f"{taking or 'none of them'}"
        )


def differences(committed, reported):
    """Name the field rather than a byte offset, because a reader has to act on it."""
    lines = []
    for key in sorted(set(committed) | set(reported)):
        if canonical(committed.get(key)) != canonical(reported.get(key)):
            if key in PER_SURFACE_KEYS or isinstance(committed.get(key), list):
                gone = [item for item in committed.get(key, []) if item not in reported.get(key, [])]
                new = [item for item in reported.get(key, []) if item not in committed.get(key, [])]
                moved = []
                if gone:
                    moved.append(f"no longer reports {gone}")
                if new:
                    moved.append(f"now reports {new}")
                lines.append(f"    {key}: {', '.join(moved)}")
            else:
                lines.append(f"    {key}: committed {committed.get(key)!r}, reports {reported.get(key)!r}")
    return "\n".join(lines)


def main():
    if len(sys.argv) < 3:
        raise SystemExit("usage: capability_manifest.py check|write <manifest> <name>=<file>...")
    mode, manifest_path, arguments = sys.argv[1], sys.argv[2], sys.argv[3:]
    answers = answers_from(arguments)
    if mode == "write":
        write(manifest_path, answers)
    elif mode == "check":
        check(manifest_path, answers)
    else:
        raise SystemExit(f"plateforce: {mode} is not check or write")


if __name__ == "__main__":
    main()
