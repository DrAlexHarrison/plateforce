#!/usr/bin/env python3
"""Prove the capability gate's refusal branches can fire.

Each case builds four answer files from CAPABILITY.json itself, mutates one, and runs
`capability_manifest.py check` expecting a refusal that names the fault. The control runs the
unmutated answers first and must pass, so a refusal here is evidence about the branch rather
than about a broken harness. Sibling of prove-parity-coverage-refuses.py, built because the
members-disagreement branch shipped exercised by no red.
"""

import copy
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "CAPABILITY.json"


def run_check(surfaces, workdir):
    arguments = []
    for name, answer in surfaces.items():
        path = Path(workdir) / f"{name}.json"
        path.write_text(json.dumps({"ok": answer}), encoding="utf-8")
        arguments.append(f"{name}={path}")
    result = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "capability_manifest.py"), "check", str(MANIFEST)]
        + arguments,
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode, result.stderr + result.stdout


def main():
    committed = json.loads(MANIFEST.read_text(encoding="utf-8"))["surfaces"]
    failures = []
    cases = 0

    def case(title, mutate, expect):
        nonlocal cases
        cases += 1
        surfaces = copy.deepcopy(committed)
        mutate(surfaces)
        with tempfile.TemporaryDirectory() as workdir:
            code, output = run_check(surfaces, workdir)
        if code == 0:
            failures.append(f"{title}: passed, and it must refuse")
        elif expect not in output:
            failures.append(f"{title}: refused without naming the fault; wanted {expect!r}")
        else:
            print(f"  refused: {title}")

    with tempfile.TemporaryDirectory() as workdir:
        code, output = run_check(copy.deepcopy(committed), workdir)
    if code != 0:
        print(f"control failed, so no case below is evidence:\n{output}", file=sys.stderr)
        raise SystemExit(1)
    print("  control: unmutated answers pass")

    case(
        "a surface names four of the five members",
        lambda s: s["python"]["acquisition"].__setitem__(
            "members", s["python"]["acquisition"]["members"][:4]
        ),
        "does not report what is committed",
    )
    case(
        "two surfaces disagree on the members while both diverge from the manifest",
        lambda s: [
            s[name]["acquisition"].__setitem__(
                "members", s[name]["acquisition"]["members"][:4 if name == "python" else 3]
            )
            for name in ("python", "r")
        ],
        "surfaces disagree on the members of acquisition",
    )
    case(
        "a surface reports the block without naming its members",
        lambda s: s["cli"]["acquisition"].pop("members"),
        "without naming its members",
    )
    case(
        "a surface reports no acquisition at all",
        lambda s: s["browser"].pop("acquisition"),
        "report no acquisition at all",
    )

    if failures:
        for failure in failures:
            print(f"plateforce: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(f"{cases} of {cases} cases were refused")


if __name__ == "__main__":
    main()
