#!/usr/bin/env python3
"""The identifiers CHANGELOG.md prints are the ones the software reports.

    python3 scripts/check-changelog-identifiers.py

The entry tells a reader to record three things beside any number they publish: the version,
the registry revision and the registry digest. A changelog that asserts a digest is the exact
shape of claim this project keeps catching in other people's documents, so it is read back
off a run rather than trusted.

The digest moves whenever any rule, parameter or citation changes, and the revision does not
have to move with it, so the two are checked apart. Needs a built binary; a missing one is a
failure rather than a pass, because a check that reads nothing reports green for the wrong
reason.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"
TRIAL = ROOT / "crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"

# The same binary the quick start guides read their facts from, so the two cannot disagree.
BINARIES = (ROOT / "target/debug/plateforce", ROOT / "target/release/plateforce")


def binary():
    for candidate in BINARIES:
        if candidate.exists():
            return candidate
    sys.exit(
        "no built plateforce binary at %s, so nothing was compared. Run `cargo build -p "
        "plateforce-cli` first." % " or ".join(str(b) for b in BINARIES)
    )


def reported(program):
    """The version the program prints, and the identifiers a real analysis record carries."""
    version = json.loads(
        subprocess.run(
            [str(program), "version", "--format", "json"],
            capture_output=True, text=True, check=True,
        ).stdout
    )["ok"]["plateforce_version"]
    record = json.loads(
        subprocess.run(
            [
                str(program), "analyse", str(TRIAL),
                "--column", "0", "--sentinel", "none", "--sample-rate-hz", "1200",
                "--preset", "sams", "--format", "json",
            ],
            capture_output=True, text=True, check=True,
        ).stdout
    )["ok"]
    return {
        "plateforce version": version,
        "method registry revision": record["registry_declared_version"],
        "method registry digest": record["registry_digest"],
    }


def stated():
    """Each identifier the changelog prints, read out of its own block."""
    text = CHANGELOG.read_text(encoding="utf-8")
    found = {}
    for label in ("plateforce version", "method registry revision", "method registry digest"):
        match = re.search(r"^%s\s+(\S+)\s*$" % re.escape(label), text, re.M)
        if match:
            found[label] = match.group(1)
    return found


program = binary()
live = reported(program)
written = stated()

missing = [label for label in live if label not in written]
if missing:
    sys.exit(
        "%s prints no line for %s, so the reader is not told what identifies their numbers"
        % (CHANGELOG.name, ", ".join(missing))
    )

print("identifiers compared: %d, read from %s" % (len(live), program.relative_to(ROOT)))
faults = []
for label, value in live.items():
    mark = "matches" if written[label] == value else "STALE"
    print("  %-26s %-26s %s" % (label, written[label], mark))
    if written[label] != value:
        faults.append("%s: the changelog says %s, the software reports %s"
                      % (label, written[label], value))

if faults:
    print()
    for fault in faults:
        print("  " + fault)
    sys.exit("%d identifier(s) in %s no longer describe this build" % (len(faults), CHANGELOG.name))
print("every identifier the changelog prints is the one the software reports")
