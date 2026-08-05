#!/usr/bin/env python3
"""Hold `docs/for-an-agent.md` to the software's own manifest.

A document describing a surface it cannot be checked against is the
assertion-rather-than-query failure this project keeps catching, and a contract written for a
program driving the terminal is the worst place to have one: nothing reads it more literally.

So every list in that document that names something the manifest also names is fenced under a
marker, and this compares the two in both directions. A list naming an operation that does not
exist sends an agent to a command that refuses; a list omitting one hides a capability from the
only reader who cannot go and look.

Exit 0 agree, 1 disagree, 9 could not decide. Nine rather than one when a marker matches nothing,
because a comparison over an empty list agrees with everything, which reads exactly like a
document that is correct.
"""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import sys

MARKER = re.compile(
    r"<!--\s*checked-against-capability:\s*(?P<field>[a-z_]+)\s*-->\s*\n```\n(?P<body>.*?)```",
    re.S,
)

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCUMENT = ROOT / "docs" / "for-an-agent.md"


def manifest() -> dict:
    """The build's own answer, taken from the binary rather than from a committed copy.

    `CAPABILITY.json` is regenerated deliberately and can lag the tree between the edit and the
    regeneration, so a document checked against it would agree with a file rather than with the
    software.
    """
    run = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "plateforce-cli",
            "--",
            "--registry",
            "registry",
            "capability",
            "--format",
            "json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if run.returncode != 0:
        print(f"plateforce: the manifest could not be read: {run.stderr.strip()[:400]}")
        sys.exit(9)
    return json.loads(run.stdout)["ok"]


def declared(text: str) -> dict[str, list[str]]:
    found: dict[str, list[str]] = {}
    for match in MARKER.finditer(text):
        body = [line.strip() for line in match.group("body").splitlines()]
        found[match.group("field")] = [line for line in body if line]
    return found


def published(answer: dict, field: str) -> list[str] | None:
    """The manifest's own list for one field, flattened to the names a reader would write."""
    if field not in answer:
        return None
    rows = answer[field]
    if not isinstance(rows, list):
        return None
    names: list[str] = []
    for row in rows:
        if isinstance(row, str):
            names.append(row)
        elif isinstance(row, dict) and "code" in row:
            names.append(row["code"])
        else:
            return None
    return names


def main() -> int:
    if not DOCUMENT.is_file():
        print(f"plateforce: {DOCUMENT} is the contract this checks and it is missing")
        return 9

    answer = manifest()
    blocks = declared(DOCUMENT.read_text())
    if not blocks:
        print(
            "plateforce: the contract carries no checked-against-capability marker, so this "
            "check compared nothing and would agree with any document"
        )
        return 9

    disagreements = 0
    for field, stated in sorted(blocks.items()):
        names = published(answer, field)
        if names is None:
            print(f"{field:16} the manifest publishes no such field, so nothing was compared")
            return 9
        if not stated:
            print(f"{field:16} the contract's block is empty, so nothing was compared")
            return 9

        invented = [name for name in stated if name not in names]
        omitted = [name for name in names if name not in stated]
        print(
            f"{field:16} contract {len(stated):3}, manifest {len(names):3}, "
            f"invented {len(invented)}, omitted {len(omitted)}"
        )
        for name in invented:
            print(f"  invented: {name}, which the manifest does not publish")
            disagreements += 1
        for name in omitted:
            print(f"  omitted:  {name}, which the manifest publishes and the contract hides")
            disagreements += 1

    if disagreements:
        print(
            f"plateforce: the contract and the manifest disagree in {disagreements} places, and "
            f"an agent reading {DOCUMENT.name} would be told something this build does not do"
        )
        return 1
    print("the contract names what the manifest names, both directions")
    return 0


if __name__ == "__main__":
    sys.exit(main())
