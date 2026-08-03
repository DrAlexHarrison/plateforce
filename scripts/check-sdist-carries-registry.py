#!/usr/bin/env python3
"""The source distribution carries every registry file the build will read.

`crates/plateforce-python/build.rs` reads the registry at compile time and embeds it, and
every wheel is built from this tarball with no checkout beside it. So a registry missing here
is not a smaller sdist, it is five platforms failing at once, and the message they fail with
is a Rust panic about a path.

Asserted against the tree's own registry rather than against a number written here, because a
count in a file goes stale the first time a rule is added and then certifies whatever arrives.
Equality rather than presence: a tarball carrying one method file is not a registry, and a
check for a non-empty directory passes on it.

Run: python3 scripts/check-sdist-carries-registry.py <sdist.tar.gz>
"""

from __future__ import annotations

import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def tracked_registry_files() -> set[str]:
    registry = ROOT / "registry"
    if not registry.is_dir():
        raise SystemExit(f"plateforce: no registry at {registry}, so there is nothing to compare")
    return {
        str(path.relative_to(registry))
        for path in registry.rglob("*")
        if path.is_file()
    }


def packaged_registry_files(archive: Path) -> set[str]:
    """Every member under the tarball's own `registry/`, keyed the way the tree keys them.

    An sdist holds one top-level directory named for the distribution, so the prefix is read
    off the member rather than assumed: a version bump renames it and a hardcoded prefix would
    then match nothing and report an empty registry as a missing one.
    """
    packaged: set[str] = set()
    with tarfile.open(archive, "r:gz") as tarball:
        for member in tarball.getmembers():
            if not member.isfile():
                continue
            parts = Path(member.name).parts
            if len(parts) < 3 or parts[1] != "registry":
                continue
            packaged.add(str(Path(*parts[2:])))
    return packaged


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: check-sdist-carries-registry.py <sdist.tar.gz>")
    archive = Path(sys.argv[1])
    if not archive.is_file():
        raise SystemExit(f"plateforce: no source distribution at {archive}")

    tracked = tracked_registry_files()
    packaged = packaged_registry_files(archive)

    missing = sorted(tracked - packaged)
    extra = sorted(packaged - tracked)
    faults = []
    if missing:
        faults.append(
            f"{len(missing)} of the {len(tracked)} registry files this tree carries are not in "
            f"{archive.name}, so a wheel built from it embeds a registry that is not this one, "
            f"starting with {missing[:5]}"
        )
    if extra:
        faults.append(
            f"{archive.name} carries {len(extra)} registry files this tree does not, so the copy "
            f"it was built from is stale: {extra[:5]}"
        )

    if faults:
        for fault in faults:
            print(f"plateforce: {fault}", file=sys.stderr)
        print(
            "plateforce: crates/plateforce-python/tools/vendor-registry.sh writes the copy the "
            "tarball is built from; run it before building the source distribution",
            file=sys.stderr,
        )
        # The copy is taken from HEAD, so a registry file added and not yet committed is
        # genuinely absent from the tarball and running the script again will not add it.
        # Saying so here is the difference between one commit and a confused half hour.
        print(
            "plateforce: that script copies from HEAD, so a registry file added and not yet "
            "committed stays absent until it is committed, or until the copy is taken with "
            "PLATEFORCE_SYNC_FROM=worktree",
            file=sys.stderr,
        )
        return 1

    print(
        f"{archive.name} carries all {len(tracked)} registry files this tree carries, "
        f"so every wheel built from it embeds this registry"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
