"""Check a wheelhouse against the platforms the release ships.

Takes the platform declaration as JSON and the directory the wheel artefacts were merged
into. Every platform must be answered by exactly one wheel, every wheel must answer for a
platform, and every wheel must carry the stable-ABI tag that makes one wheel serve every
supported interpreter.

    python3 scripts/check-wheel-set.py "$PLATFORMS" wheelhouse
"""

from __future__ import annotations

import fnmatch
import json
import sys
from pathlib import Path

STABLE_ABI_TAG = "cp311-abi3"


def interpreter_abi_and_platform_tags(wheel: Path) -> tuple[str, str, str]:
    """The last three fields of a wheel filename, which is where its tags live."""
    fields = wheel.name[: -len(".whl")].split("-")
    if len(fields) < 5:
        raise SystemExit(f"wheels: {wheel.name} does not carry the five wheel filename fields")
    return fields[-3], fields[-2], fields[-1]


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    platforms = json.loads(argv[1])
    wheels = sorted(Path(argv[2]).glob("*.whl"))

    for wheel in wheels:
        print(wheel.name)

    problems: list[str] = []
    answered_by: dict[str, list[str]] = {}

    for platform in platforms:
        pattern = platform["wheel_tag"]
        matched = [
            wheel
            for wheel in wheels
            if fnmatch.fnmatchcase(interpreter_abi_and_platform_tags(wheel)[2], pattern)
        ]
        for wheel in matched:
            answered_by.setdefault(wheel.name, []).append(platform["platform"])
        if len(matched) != 1:
            problems.append(
                f"{platform['platform']} wants one wheel tagged {pattern}, {len(matched)} arrived"
            )

    for wheel in wheels:
        answers_for = answered_by.get(wheel.name, [])
        if not answers_for:
            problems.append(f"{wheel.name} is tagged for no platform this release ships")
        elif len(answers_for) > 1:
            problems.append(f"{wheel.name} answers for {' and '.join(answers_for)}")

        interpreter_tag, abi_tag, _ = interpreter_abi_and_platform_tags(wheel)
        if f"{interpreter_tag}-{abi_tag}" != STABLE_ABI_TAG:
            problems.append(
                f"{wheel.name} is tagged {interpreter_tag}-{abi_tag} rather than {STABLE_ABI_TAG}, "
                "which is one wheel per interpreter rather than one per platform"
            )

    if problems:
        # The two streams share one log, so the list of what arrived has to land before the
        # complaint about it rather than after whenever the buffer happens to drain.
        sys.stdout.flush()
        for problem in problems:
            print(f"wheels: {problem}", file=sys.stderr)
        return 1

    print(f"{len(wheels)} wheels, one for each of {len(platforms)} platforms, every one {STABLE_ABI_TAG}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
