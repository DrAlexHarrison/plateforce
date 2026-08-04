#!/usr/bin/env python3
"""Show that the guards in the operators test refuse, one break at a time.

green, break, red, restore, green, run against the working tree. The file it breaks is
restored from HEAD after every case, so uncommitted work in it is lost: commit first.

Every case asserts its anchor is in the file before editing, asserts the edit landed, prints
`applied <name>`, and aborts rather than running a test whose result could not be read. A break
that never applied and a break the guard survived both print a green line.

The result line is read for the test COUNT and the `filtered out` number, never for the word
ok: a bare name filter applies to every target in the command.

    python3 scripts/prove-operator-guards-refuse.py
"""

import pathlib
import re
import subprocess
import sys

ROOT = str(pathlib.Path(__file__).resolve().parent.parent)
FILE = "crates/plateforce-analysis/tests/the_operators_a_default_request_composes.rs"
TEST = "the_operators_a_default_request_composes"
PATH = f"{ROOT}/{FILE}"


def shell(command):
    return subprocess.run(command, cwd=ROOT, shell=True, capture_output=True, text=True)


def abort(why):
    print(f"ABORT: {why}", file=sys.stderr)
    shell(f"git checkout HEAD -- {FILE}")
    raise SystemExit(1)


def measure():
    """Run the file's tests and return (exit status, output, the result line)."""
    done = shell(f"cargo test -q -p plateforce-analysis --test {TEST}")
    output = done.stdout + done.stderr
    lines = [line for line in output.splitlines() if line.startswith("test result:")]
    result = lines[-1] if lines else "NO RESULT LINE AT ALL"
    print(f"  {result}")
    return done.returncode, output, result


def green(step):
    status, output, result = measure()
    if status != 0:
        print(output, file=sys.stderr)
        abort(f"{step} is not green, so nothing after it means anything")
    if not re.search(r"[1-9]\d* passed", result):
        abort(f"{step} is green with no test having run: {result}")
    if "0 filtered out" not in result:
        abort(f"{step} filtered a test out, so the count is not this file's: {result}")
    return result


def apply(anchor, replacement, name):
    text = open(PATH, encoding="utf-8").read()
    if anchor not in text:
        abort(f"the anchor for {name} is not in the file: {anchor[:60]!r}")
    open(PATH, "w", encoding="utf-8").write(text.replace(anchor, replacement, 1))
    landed = open(PATH, encoding="utf-8").read()
    if replacement not in landed:
        abort(f"{name} did not apply")
    # A break that keeps its anchor is a prepend, so the anchor surviving is expected there and
    # is evidence the edit was skipped anywhere else.
    if anchor not in replacement and anchor in landed:
        abort(f"{name} did not apply, the anchor is still in the file")
    print(f"applied {name}")


def restore(after):
    if shell(f"git checkout HEAD -- {FILE}").returncode != 0:
        abort(f"the restore after {after} failed")
    if shell(f"git status --porcelain -- {FILE}").stdout.strip():
        abort(f"the restore after {after} did not land")
    green(f"the run after restoring {after}")


CASES = [
    (
        "break 1, the reader reads onset.op. again, the defect itself",
        'fn is_operator(method_id: &str) -> bool {\n    method_id.contains(".op.")\n}',
        'fn is_operator(method_id: &str) -> bool {\n    method_id.starts_with("onset.op.")\n}',
        "so every count in this file is",
        "the narrowed reader",
    ),
    (
        "break 2, the response stops recording one construct's operators",
        "        .filter(|bound| is_operator(&bound.method_id))",
        "        .filter(|bound| is_operator(&bound.method_id)\n"
        '            && !bound.method_id.starts_with("takeoff"))',
        "the registry declares operators for",
        "the population every count is taken over",
    ),
    (
        "break 3, two constructs anchor their search in different places",
        "    let mut anchors: BTreeMap<&str, Vec<&String>> = BTreeMap::new();",
        '    let mut operators = operators.clone();\n'
        '    operators.retain(|id| id != "takeoff.op.search_floor_at_weighing_epoch_end");\n'
        '    operators.push("takeoff.op.search_floor_at_trial_start".to_string());\n'
        "    let mut anchors: BTreeMap<&str, Vec<&String>> = BTreeMap::new();",
        "began searching from different places",
        "the anchors",
    ),
    (
        "break 4, a bare request composes a deprecated rule",
        "    let deprecated: Vec<&String> = operators",
        "    let mut operators = operators.clone();\n"
        '    operators.push("onset.op.search_floor".to_string());\n'
        "    let deprecated: Vec<&String> = operators",
        "are deprecated",
        "the deprecated rule",
    ),
    (
        "break 5, only one construct has a floor to choose between, the vacuity case",
        '        .filter(|id| is_operator(id) && id.contains("search_floor"))',
        '        .filter(|id| id.contains("onset.op.search_floor"))',
        "asserts nothing about a choice",
        "the population of choices",
    ),
]


def main():
    if shell(f"git status --porcelain -- {FILE}").stdout.strip():
        abort(f"{FILE} is already modified, so a restore could not be told from somebody's edit")

    print("=== step 1, green, the step everybody skips ===")
    green("step 1")

    caught = 0
    for name, anchor, replacement, expected, about in CASES:
        print(f"=== {name} ===")
        apply(anchor, replacement, name)
        status, output, result = measure()
        if status == 0:
            print(output, file=sys.stderr)
            abort(f"THE BREAK SURVIVED: {name} left every test passing")
        if expected not in output:
            print(output, file=sys.stderr)
            abort(f"{name} went red for some other reason than {about}")
        print(f"  red, and about {about}")
        caught += 1
        restore(name)

    print()
    print(f"{caught} of {len(CASES)} breaks were caught, and the file came back green after each")


if __name__ == "__main__":
    main()
