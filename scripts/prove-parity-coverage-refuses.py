#!/usr/bin/env python3
"""Show that the coverage refusal in `result_parity.py` fires, one cause at a time.

A gate whose coverage list was narrower than the document it guarded is what this proves is
fixed, and a proof of that kind is worth nothing unless the refusal is watched failing. Each
case below starts from four answers that pass, changes one thing, and requires the named
refusal. A case that produced no fault is reported as a case that proved nothing, because a
gate that cannot refuse and a gate with nothing to refuse read identically from the outside.

The four answers are the ones the surfaces actually computed, read from the committed
baseline and reshaped into the per-surface documents the harness collects. So a case is a
change against measured input rather than against a document written here.

Run it after any edit to `result_parity.py`, and after any change to what a surface publishes:

    python3 scripts/prove-parity-coverage-refuses.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))

import result_parity as gate

ROOT = pathlib.Path(__file__).parent.parent
BASELINE = ROOT / "tests" / "golden" / "result-parity.json"

# What each surface publishes beyond the compared fields, measured on 2026-08-03 by running
# the four arms and taking the top-level keys of each answer. Held here as names alone: the
# cases below turn on which surface carries a field, never on its value.
BEYOND_THE_COMPARED_FIELDS = {
    "cli": ["plateforce_version", "registry_digest", "registry_version", "spread", "trial"],
    "browser": ["plateforce_version", "registry_digest", "registry_version", "trial"],
    "python": ["registry_digest", "registry_version"],
    "r": ["registry_digest", "descriptions"],
}


def answers_that_pass():
    """The four surfaces as they answer today, assembled from the committed result."""
    document = json.loads(BASELINE.read_text(encoding="utf-8"))
    result = document["result"]
    answers = {}
    for surface, extra in BEYOND_THE_COMPARED_FIELDS.items():
        answer = dict(result)
        for field in extra:
            # A placeholder for every field this comparison does not reach, except the digest,
            # which the assertion that covers it reads.
            answer[field] = "content-0" if field == "registry_digest" else f"{surface}-{field}"
        answers[surface] = answer
    return answers, document["compared_fields"]


def faults_when(name, change, expected):
    """Apply one change to a passing run and require a fault that names `expected`."""
    answers, fields = answers_that_pass()
    fields = list(fields)
    fields = change(answers, fields) or fields
    print(f"applied {name}", flush=True)

    faults = gate.coverage_faults(answers, fields)
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
]


def main():
    if not a_control_that_must_pass():
        raise SystemExit(1)

    survived = [name for name, change, expected in CASES if not faults_when(name, change, expected)]
    print()
    print(f"{len(CASES) - len(survived)} of {len(CASES)} cases were refused")
    if survived:
        for name in survived:
            print(f"plateforce: {name} did not make the gate refuse", file=sys.stderr)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
