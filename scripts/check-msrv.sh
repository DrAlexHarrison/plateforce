#!/usr/bin/env bash
# Compare the declared workspace rust-version against the highest one any dependency
# declares. A dependency bump that raises the real floor above the declared one is the
# defect that gets Rust packages archived from CRAN, and it is invisible until someone
# builds on an older toolchain.
set -euo pipefail

cargo metadata --format-version 1 --locked | python3 -c '
import json, sys

def as_numbers(version):
    parts = [int(part) for part in version.split(".")]
    return tuple(parts + [0] * (3 - len(parts)))

metadata = json.load(sys.stdin)
workspace_members = set(metadata["workspace_members"])
declared = next(
    package["rust_version"]
    for package in metadata["packages"]
    if package["id"] in workspace_members and package.get("rust_version")
)

dependencies = [
    (package["rust_version"], package["name"])
    for package in metadata["packages"]
    if package.get("rust_version") and package["id"] not in workspace_members
]
required_version, required_by = max(dependencies, key=lambda row: as_numbers(row[0]))

if as_numbers(required_version) > as_numbers(declared):
    print(
        f"workspace declares rust-version {declared}, but {required_by} needs {required_version}\n"
        f"raise the workspace rust-version, or hold {required_by} at an older version",
        file=sys.stderr,
    )
    raise SystemExit(1)

print(f"rust-version {declared} covers every dependency, highest is {required_by} at {required_version}")
'
