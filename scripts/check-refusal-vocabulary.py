#!/usr/bin/env python3
"""Every refusal code a surface can raise, against the vocabulary it publishes.

`CAPABILITY.json` is each surface's statement of the codes it can emit, and an R condition
class or a JSON `code` field outside that list is a class a caller can catch and a manifest
says does not exist. The count is a query rather than a number in a document, and it is held
to a ceiling that can only fall.

The ceiling is a file rather than a constant here, so lowering it is a diff a reader can see.

Run: python3 scripts/check-refusal-vocabulary.py [--write]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CEILING = ROOT / "scripts" / "refusal-vocabulary-ceiling.txt"

# One entry per surface that builds a refusal code from a string rather than from
# `RefusalCode`. A surface that names its codes through the enum cannot drift and is not
# listed. The pattern tolerates a line wrap between the call and its first argument: a
# pattern anchored to one line counted nine of these as four.
SOURCES = {
    "r": (
        ROOT / "bindings/r/src/rust/src/lib.rs",
        re.compile(r'Refusal::(?:of|naming_parameter)\(\s*"([a-z_]+)"'),
    ),
}


def published() -> dict[str, set[str]]:
    manifest = json.loads((ROOT / "CAPABILITY.json").read_text())
    return {
        name: {row["code"] for row in body.get("refusal_codes", [])}
        for name, body in manifest["surfaces"].items()
    }


def read_ceiling() -> dict[str, int]:
    if not CEILING.exists():
        return {}
    ceiling = {}
    for line in CEILING.read_text().splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        surface, count = line.split()
        ceiling[surface] = int(count)
    return ceiling


def main() -> int:
    write = "--write" in sys.argv
    vocabularies = published()
    ceiling = read_ceiling()
    measured: dict[str, int] = {}
    failures: list[str] = []

    for surface, (path, pattern) in SOURCES.items():
        if surface not in vocabularies:
            failures.append(f"{surface} builds codes and the manifest carries no vocabulary for it")
            continue
        built = set(pattern.findall(path.read_text()))
        inside = sorted(built & vocabularies[surface])
        outside = sorted(built - vocabularies[surface])
        measured[surface] = len(outside)
        print(
            f"{surface}: builds {len(built)} codes, {len(outside)} outside the "
            f"{len(vocabularies[surface])} its manifest publishes"
        )
        for code in outside:
            print(f"    {code}")
        # A control that must return hits. Zero here means the pattern stopped matching and
        # the run above measured an empty set rather than a clean surface.
        if not inside:
            failures.append(
                f"{surface}: no code matched the published vocabulary, so the pattern is "
                f"reading nothing rather than finding nothing"
            )
        allowed = ceiling.get(surface)
        if allowed is None:
            failures.append(f"{surface} has no ceiling recorded in {CEILING.name}")
        elif len(outside) > allowed:
            failures.append(
                f"{surface} raises {len(outside)} codes outside its vocabulary, "
                f"against a ceiling of {allowed}"
            )

    if write:
        CEILING.write_text(
            "# Refusal codes each surface can raise that its own manifest does not publish.\n"
            "# The ceiling only falls. Lower it in the commit that closes a code.\n"
            + "".join(f"{surface} {count}\n" for surface, count in sorted(measured.items()))
        )
        print(f"wrote {CEILING.relative_to(ROOT)}")
        return 0

    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
