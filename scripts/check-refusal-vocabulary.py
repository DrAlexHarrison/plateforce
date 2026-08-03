#!/usr/bin/env python3
"""Every refusal code a surface can raise, against the vocabulary it publishes.

`CAPABILITY.json` is each surface's statement of the codes it can emit, and an R condition
class or a JSON `code` field outside that list is a class a caller can catch and a manifest
says does not exist. The count is a query rather than a number in a document, and it is held
to a ceiling that has to equal it. A ceiling standing above the count it permits admits that
many new codes with this gate green, so slack is the failure rather than the margin.

The ceiling is a file rather than a constant here, so lowering it is a diff a reader can see,
and the rule the file states is the rule this reads back rather than one spelled again here.

Run: python3 scripts/check-refusal-vocabulary.py [--write]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CEILING = ROOT / "scripts" / "refusal-vocabulary-ceiling.txt"

# Every place a surface builds a refusal code from a string rather than from `RefusalCode`.
# A surface that names its codes through the enum cannot drift and is not listed.
#
# Per surface, one group per body of source that can raise, because a surface raises from
# more than one language. Reading a single file reported the R package at 7 codes outside
# its vocabulary while its own `.R` sources raised 8 more that no manifest carries. A gate
# corrected once for reading the wrong pattern had never been asked whether it reads the
# right files, which is the same fault one level up.
#
# Every pattern tolerates a line wrap between a call and its first argument: a pattern
# anchored to one line counted nine of these as four.
SOURCES = {
    "r": (
        # The Rust shim, which answers R with a JSON envelope carrying the code as a field.
        (
            "the Rust shim",
            ["bindings/r/src/rust/src/lib.rs"],
            [re.compile(r'Refusal::(?:of|naming_parameter)\(\s*"([a-z_]+)"')],
        ),
        # The package's own R sources, which raise a classed condition without crossing into
        # Rust at all. Two shapes: the code as the first argument to this package's raiser,
        # and the code as a named field on a condition built where it is raised.
        (
            "the R sources",
            sorted(
                str(path.relative_to(ROOT))
                for path in (ROOT / "bindings/r/R").glob("*.R")
            ),
            [
                re.compile(r'refuse_here\(\s*"([a-z_]+)"', re.S),
                re.compile(r'\bcode\s*=\s*"([a-z_]+)"'),
            ],
        ),
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


def read_header() -> str:
    """The rule the ceiling is held to lives in the file it governs, and `--write` carries it.

    Re-emitting a copy written here would let a rewrite revert a ruling the file already
    carries: the text amended in one commit is lost in the next, and the run that lost it
    prints success. So the leading comment block is read and written back unchanged, and a
    file that carries no rule at all is a fault rather than an invitation to invent one.
    """
    header: list[str] = []
    for line in CEILING.read_text().splitlines() if CEILING.exists() else []:
        if line.startswith("#") or not line.strip():
            header.append(line)
            continue
        break
    if not any(line.startswith("#") for line in header):
        raise SystemExit(
            f"plateforce: {CEILING.name} states no rule for the numbers it carries, and which "
            "rule a ceiling is held to is a decision rather than a measurement"
        )
    return "\n".join(header) + "\n"


def main() -> int:
    write = "--write" in sys.argv
    vocabularies = published()
    ceiling = read_ceiling()
    measured: dict[str, int] = {}
    failures: list[str] = []

    for surface, groups in SOURCES.items():
        if surface not in vocabularies:
            failures.append(f"{surface} builds codes and the manifest carries no vocabulary for it")
            continue
        built: set[str] = set()
        for label, paths, patterns in groups:
            read: set[str] = set()
            for name in paths:
                text = (ROOT / name).read_text()
                for pattern in patterns:
                    read |= set(pattern.findall(text))
            print(f"{surface}, {label}: {len(read)} codes across {len(paths)} files")
            # A control per group, not per surface. A surface reads from several bodies of
            # source, and one group still matching hides another that has stopped: the
            # count then looks like a clean body of source rather than a blind one.
            if not read:
                failures.append(
                    f"{surface}, {label}: matched no code at all across {len(paths)} files, "
                    f"so the pattern is reading nothing rather than finding nothing"
                )
            built |= read
        inside = sorted(built & vocabularies[surface])
        outside = sorted(built - vocabularies[surface])
        measured[surface] = len(outside)
        print(
            f"{surface}: builds {len(built)} codes, {len(outside)} outside the "
            f"{len(vocabularies[surface])} its manifest publishes"
        )
        for code in outside:
            print(f"    {code}")
        # A second control, over the surface. Zero here means every pattern matched
        # something that no manifest carries, which is a vocabulary nobody is reading.
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
        elif len(outside) < allowed:
            # Slack is the failure, not the margin. A ceiling standing above the count it
            # permits admits that many new codes with the gate green, which is how two
            # arrived unnoticed once already. The number to write is named here so closing
            # a code and lowering the ceiling stay one commit.
            failures.append(
                f"{surface} raises {len(outside)} codes outside its vocabulary against a "
                f"ceiling of {allowed}, so {allowed - len(outside)} more could arrive with "
                f"this gate green: write `{surface} {len(outside)}` in {CEILING.name}"
            )

    if write:
        CEILING.write_text(
            read_header()
            + "".join(f"{surface} {count}\n" for surface, count in sorted(measured.items()))
        )
        print(f"wrote {CEILING.relative_to(ROOT)}")
        return 0

    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
