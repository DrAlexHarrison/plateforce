#!/usr/bin/env python3
"""Writes the dependency licence tables into NOTICE, or checks that they are current.

    python3 scripts/write-dependency-licences.py            write
    python3 scripts/write-dependency-licences.py --check    fail if the file is stale

Two tables, never one. The published crates and the desktop shell are separate
distributions with separate audiences, and adding their package counts together would sum
two populations that answer different questions.
"""

import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parent.parent
NOTICE = REPOSITORY / "NOTICE"

HEADING = "Dependency license posture"
RULE = "=" * 80


def resolved_dependencies(manifest: Path) -> Counter:
    """Every package the manifest resolves, minus the ones this repository publishes."""
    raw = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(manifest)],
        check=True,
        capture_output=True,
        text=True,
        cwd=REPOSITORY,
    ).stdout
    metadata = json.loads(raw)
    ours = set(metadata["workspace_members"])

    licences = Counter()
    for package in metadata["packages"]:
        if package["id"] in ours:
            continue
        declared = package.get("license")
        if not declared:
            file = package.get("license_file")
            declared = f"declared in {file}" if file else "no licence declared"
        licences[declared] += 1
    return licences


def as_table(licences: Counter) -> list[str]:
    total = sum(licences.values())
    return [
        f"  {declared:<52}{count:>4} of {total}"
        for declared, count in sorted(licences.items(), key=lambda row: (-row[1], row[0]))
    ]


def section() -> str:
    published = resolved_dependencies(REPOSITORY / "Cargo.toml")
    shell = resolved_dependencies(REPOSITORY / "src-tauri" / "Cargo.toml")

    lines = [
        f"The published crates resolve {sum(published.values())} packages beyond the ones this",
        "repository publishes, grouped here by the license each one declares:",
        "",
        *as_table(published),
        "",
        f"The desktop shell resolves {sum(shell.values())} packages. It is a separate distribution",
        "with a separate audience, so its dependencies are counted apart and the two totals",
        "are never added together:",
        "",
        *as_table(shell),
        "",
        "Both tables are written by scripts/write-dependency-licences.py from what cargo",
        "resolved, and CI runs it with --check, so a dependency added without its license",
        "recorded fails the build rather than shipping.",
    ]
    return "\n".join(lines)


def rewritten(current: str, body: str) -> str:
    pattern = re.compile(
        rf"({re.escape(RULE)}\n{re.escape(HEADING)}\n{re.escape(RULE)}\n\n).*?(\n\n{re.escape(RULE)})",
        re.DOTALL,
    )
    if not pattern.search(current):
        raise SystemExit(f"NOTICE has no '{HEADING}' section to write into")
    return pattern.sub(lambda m: f"{m.group(1)}{body}{m.group(2)}", current, count=1)


def main() -> int:
    checking = "--check" in sys.argv[1:]
    current = NOTICE.read_text()
    wanted = rewritten(current, section())

    if not checking:
        NOTICE.write_text(wanted)
        print("NOTICE rewritten")
        return 0

    if current != wanted:
        print(
            "NOTICE is stale: a dependency changed and its license is not recorded.\n"
            "Run scripts/write-dependency-licences.py",
            file=sys.stderr,
        )
        for line in set(wanted.splitlines()) - set(current.splitlines()):
            print(f"  now:{line}", file=sys.stderr)
        return 1

    print("NOTICE is current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
