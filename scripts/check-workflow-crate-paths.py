#!/usr/bin/env python3
"""A workflow's path filter covers every crate whose change could move what it verifies.

A gate that does not run is a gate that passed, and a `paths:` list is a hand-written claim
about a dependency graph that nobody re-reads when the graph moves. Two silent holes in one
workflow: `crates/plateforce-analysis/**` was missing until 20519b5, and
`crates/plateforce-batch/**` until 2026-08-04, one crate over from that fix. In both a push to
`main` could move every number Python reports while the job that tests Python sat out.

So the list is checked against the dependency closure read out of the manifests rather than
against anybody's memory. A workflow declares its root below, the closure is computed, and a
crate in the closure with no covering path entry fails the gate.

Broad filters are honest and exempt: a workflow matching `crates/**`, or carrying no `paths:`
under `push:` at all, already runs on any crate change and needs nothing from this.

Run: python3 scripts/check-workflow-crate-paths.py
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# What each workflow is verifying, as a manifest whose plateforce dependencies start the
# closure. A workflow absent from here is not checked, so adding a gate that reads crates
# means adding it here too.
ROOTS = {
    "python-wheels.yml": "crates/plateforce-python/Cargo.toml",
    "r-package.yml": "bindings/r/src/rust/Cargo.toml",
    # plateforce-cli, because that is the package scripts/build-serve-binaries.sh builds and
    # the static binaries are what this workflow ships. Named as plateforce-serve until now,
    # which depends on no workspace member, so the row's closure was empty and it reported
    # that it was asserting nothing while five crates went uncovered.
    "desktop-linux.yml": "crates/plateforce-cli/Cargo.toml",
}

DEP_LINE = re.compile(r"^\s*(plateforce-[a-z0-9-]+)\s*=", re.MULTILINE)


def workspace_members() -> dict[str, Path]:
    """Every workspace crate and the directory holding its manifest."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    meta = json.loads(out.stdout)
    return {p["name"]: Path(p["manifest_path"]).parent for p in meta["packages"]}


def named_dependencies(manifest: Path) -> set[str]:
    """The plateforce crates a manifest names, however it spells the source."""
    if not manifest.exists():
        return set()
    text = manifest.read_text(encoding="utf-8")
    # The package's own name is not a dependency on itself.
    own = re.search(r'^\s*name\s*=\s*"([^"]+)"', text, re.MULTILINE)
    found = set(DEP_LINE.findall(text))
    if own:
        found.discard(own.group(1))
    return found


def closure(manifest: Path, members: dict[str, Path]) -> set[str]:
    """Every workspace crate reachable from a manifest, transitively."""
    seen: set[str] = set()
    pending = list(named_dependencies(manifest))
    while pending:
        name = pending.pop()
        if name in seen or name not in members:
            continue
        seen.add(name)
        pending.extend(named_dependencies(members[name] / "Cargo.toml"))
    return seen


def trigger_paths(workflow: Path, trigger: str) -> list[str] | None | bool:
    """The `paths:` list under one trigger: False when the workflow has no such trigger,
    None when it has one that declares no paths, the list otherwise.

    Parsed by structure rather than by a search for the word, because several of these files
    carry a `paths:` under more than one trigger and matching the wrong one would read a
    covered workflow as uncovered, or the reverse.
    """
    lines = workflow.read_text(encoding="utf-8").splitlines()
    inside = False
    in_paths = False
    collected: list[str] = []
    for line in lines:
        if re.match(rf"^\s{{0,4}}{trigger}:\s*$", line):
            inside, in_paths = True, False
            continue
        # Any other trigger at the same indent ends the block.
        if inside and re.match(r"^\s{0,4}[a-z_]+:\s*$", line) and not line.strip().startswith(("paths", "branches")):
            break
        if inside and re.match(r"^\s+paths:\s*$", line):
            in_paths = True
            continue
        if in_paths:
            # A comment or a blank line inside the list is not the end of it. Treating one as
            # the end truncates the filter and reports covered crates as uncovered, which is
            # loud rather than silent and is still wrong.
            if not line.strip() or line.strip().startswith("#"):
                continue
            item = re.match(r"^\s+-\s+[\"']?([^\"'\s]+)[\"']?\s*$", line)
            if item:
                collected.append(item.group(1))
            else:
                in_paths = False
    if not inside:
        return False
    return collected or None


def filtering_trigger(workflow: Path) -> tuple[str, list[str] | None]:
    """Which trigger's filter decides whether this workflow runs, and what it holds.

    `push` where there is one. `desktop-linux.yml` has none and runs on `pull_request`, and
    reading its absent push filter as an empty list reported every crate in the closure as
    uncovered, which is the opposite of the exemption the absent trigger deserves.
    """
    for trigger in ("push", "pull_request"):
        found = trigger_paths(workflow, trigger)
        if found is not False:
            return trigger, found
    return "none", None


def covered(crate: str, patterns: list[str], members: dict[str, Path]) -> bool:
    directory = members[crate].relative_to(ROOT).as_posix()
    for pattern in patterns:
        if pattern in (f"{directory}/**", f"{directory}/*", directory):
            return True
        if pattern in ("crates/**", "**"):
            return True
    return False


def main() -> int:
    members = workspace_members()
    failures: list[str] = []
    checked = 0
    asserted = 0          # rows whose closure is non-empty, so the row could have failed

    for name, manifest_path in sorted(ROOTS.items()):
        workflow = ROOT / ".github" / "workflows" / name
        if not workflow.exists():
            failures.append(f"{name}: declared here but absent from .github/workflows/")
            continue
        trigger, patterns = filtering_trigger(workflow)
        needed = sorted(closure(ROOT / manifest_path, members))
        if patterns is None:
            print(f"{name:24s} {trigger} carries no paths filter, so every crate change runs it")
            checked += 1
            continue
        checked += 1
        missing = [c for c in needed if not covered(c, patterns, members)]
        if not needed:
            # A population of zero passing is this project's named anti-pattern, so it says
            # so rather than printing ok. plateforce-serve depends on no workspace crate
            # today, so this row cannot fail and proves nothing; it starts working the day
            # serve gains one, which is the reason to keep it rather than to trust it.
            state = "closure is empty, so this row asserts nothing today"
        else:
            asserted += 1
            state = "ok" if not missing else "MISSING " + ", ".join(missing)
        print(f"{name:24s} closure {len(needed)}: {', '.join(needed) or 'none'}  {state}")
        for crate in missing:
            directory = members[crate].relative_to(ROOT).as_posix()
            failures.append(
                f"{name}: {crate} is in the dependency closure of {manifest_path} and no "
                f"{trigger} path covers it. Add \"{directory}/**\"."
            )

    if checked != len(ROOTS):
        failures.append(f"checked {checked} of {len(ROOTS)} declared workflows")

    # Two numbers rather than one. A reader who sees only "3 of 3 checked" is given a
    # denominator whose members did not all do the same amount of work, and one of them
    # currently cannot fail. The second number is the one that says what was established.
    print(
        f"\n{checked} of {len(ROOTS)} declared workflows read, "
        f"{asserted} of them with a non-empty closure this run could have failed on"
    )
    if failures:
        print("\nfailures:")
        for line in failures:
            print(f"  {line}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
