#!/usr/bin/env bash
# Compare the declared workspace rust-version against the highest one any dependency
# declares. A dependency bump that raises the real floor above the declared one is the
# defect that gets Rust packages archived from CRAN, and it is invisible until someone
# builds on an older toolchain.
#
# `--print-declared-floor` prints the declared version and nothing else, so CI installs the
# toolchain the manifest names rather than one written down a second time.
set -euo pipefail

mode="${1:-compare}"
case "$mode" in
  compare | --print-declared-floor) ;;
  *)
    echo "usage: $0 [--print-declared-floor]" >&2
    exit 2
    ;;
esac

cargo metadata --format-version 1 --locked | python3 -c '
import json, sys

mode = sys.argv[1]

def as_numbers(version):
    parts = [int(part) for part in version.split(".")]
    return tuple(parts + [0] * (3 - len(parts)))

metadata = json.load(sys.stdin)
workspace_members = set(metadata["workspace_members"])
members = [
    package for package in metadata["packages"] if package["id"] in workspace_members
]

# One crate quietly declaring a lower floor than its siblings publishes a promise the
# workspace does not keep, and the comparison below would never see it.
declarations = {package["name"]: package.get("rust_version") for package in members}
distinct = set(declarations.values())
if len(distinct) != 1 or None in distinct:
    listing = "\n".join(
        f"  {name}: {version}" if version else f"  {name}: no rust-version at all"
        for name, version in sorted(declarations.items())
    )
    print(
        f"the {len(members)} workspace crates do not agree on the toolchain floor:\n{listing}",
        file=sys.stderr,
    )
    raise SystemExit(1)

declared = distinct.pop()

if mode == "--print-declared-floor":
    print(declared)
    raise SystemExit(0)

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
print(f"all {len(members)} workspace crates declare {declared}")
' "$mode"
