#!/usr/bin/env python3
"""Hold every surface's answer about itself, and assert what has to be true across them.

Which entry points a surface dispatches, and which containers it can write a result into,
are facts about that surface. A comparison demanding one identical document from all of them
is satisfied only by a surface claiming something it cannot do, so each answer is recorded
under its own name and the obligations are asserted over the set instead.

Four assertions, and each fails for a different reason a reader can act on. A surface's own
answer must match what is committed for it, in both directions, so a lost entry point and an
undeclared one are both a diff. The engine's own tables must read the same everywhere, which
is what a stale build breaks. Every surface must reach every operation named in
`required_operations`. And that array must itself name operations some surface reaches, so a
misspelling is reported as a malformed obligation rather than as every surface failing.
"""

import json
import sys

# Facts about the engine every surface links, rather than about the surface that answered.
SHARED_KEYS = ("methods", "plateforce_version", "refusal_codes", "schema")

PER_SURFACE_KEYS = ("operations", "output_formats")


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

    for key in SHARED_KEYS:
        spellings = {name: canonical(answer.get(key)) for name, answer in answers.items()}
        if len(set(spellings.values())) > 1:
            grouped = sorted({value: name for name, value in spellings.items()}.values())
            faults.append(f"surfaces disagree on {key}, which every one of them links: {grouped}")

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
        f"{manifest_path}, each owing {len(required)} operations"
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
