#!/usr/bin/env python3
"""A workflow's path filter covers every crate whose change could move what it verifies.

A gate that does not run is a gate that passed, and a `paths:` list is a hand-written claim
about a dependency graph that nobody re-reads when the graph moves. This has now cost two
silent holes in two days, both in the same workflow: `crates/plateforce-analysis/**` was
missing until 20519b5, and `crates/plateforce-batch/**` until 2026-08-04, one crate over from
that fix. In both a push to `main` could move every number Python reports while the job that
tests Python sat out.

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
    "desktop-linux.yml": "crates/plateforce-serve/Cargo.toml",
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


def push_paths(workflow: Path) -> list[str] | None:
    """The `paths:` list under `push:`, or None when the trigger declares none.

    Parsed by structure rather than by a search for the word, because `pull_request:` carries
    its own `paths:` in several of these files and matching the wrong one would read a
    covered workflow as uncovered, or the reverse.
    """
    lines = workflow.read_text(encoding="utf-8").splitlines()
    in_push = False
    in_paths = False
    collected: list[str] = []
    for line in lines:
        if re.match(r"^\s{0,4}push:\s*$", line):
            in_push, in_paths = True, False
            continue
        # Any other trigger at the same indent ends the push block.
        if in_push and re.match(r"^\s{0,4}[a-z_]+:\s*$", line) and not line.strip().startswith(("paths", "branches")):
            break
        if in_push and re.match(r"^\s+paths:\s*$", line):
            in_paths = True
            continue
        if in_paths:
            item = re.match(r"^\s+-\s+[\"']?([^\"'\s]+)[\"']?\s*$", line)
            if item:
                collected.append(item.group(1))
            elif line.strip():
                in_paths = False
    if not in_push:
        return []          # no push trigger at all, nothing to under-cover
    return collected or None


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

    for name, manifest_path in sorted(ROOTS.items()):
        workflow = ROOT / ".github" / "workflows" / name
        if not workflow.exists():
            failures.append(f"{name}: declared here but absent from .github/workflows/")
            continue
        patterns = push_paths(workflow)
        needed = sorted(closure(ROOT / manifest_path, members))
        if patterns is None:
            print(f"{name:24s} push carries no paths filter, so every crate change runs it")
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
            state = "ok" if not missing else "MISSING " + ", ".join(missing)
        print(f"{name:24s} closure {len(needed)}: {', '.join(needed) or 'none'}  {state}")
        for crate in missing:
            directory = members[crate].relative_to(ROOT).as_posix()
            failures.append(
                f"{name}: {crate} is in the dependency closure of {manifest_path} and no "
                f"push path covers it. Add \"{directory}/**\"."
            )

    if checked != len(ROOTS):
        failures.append(f"checked {checked} of {len(ROOTS)} declared workflows")

    print(f"\n{checked} of {len(ROOTS)} declared workflows checked against their closures")
    if failures:
        print("\nfailures:")
        for line in failures:
            print(f"  {line}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
