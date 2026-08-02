#!/usr/bin/env bash
# Compare the declared workspace rust-version against the highest one any dependency
# declares. A dependency bump that raises the real floor above the declared one is the
# defect that gets Rust packages archived from CRAN, and it is invisible until someone
# builds on an older toolchain.
#
# The comparison is taken per target this project ships for, because `cargo metadata`
# without a platform filter reports floors for targets nobody compiles. One resolution
# reached `wasip2`, which declares 1.87 and is built only for `wasm32-wasip2`, and the gate
# failed on a toolchain no surface of this project uses.
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

# The four targets a plateforce artifact is built for: three desktop platforms and the
# browser. A floor reachable on none of them is a floor this project never meets.
readonly SHIPPED_TARGETS=(
  x86_64-unknown-linux-gnu
  aarch64-apple-darwin
  x86_64-pc-windows-msvc
  wasm32-unknown-unknown
)

readonly WORK="$(mktemp -d)"
trap 'command rm -rf "${WORK}"' EXIT

cargo metadata --format-version 1 --locked > "${WORK}/unfiltered.json"

# Resolving four targets costs four cargo invocations, so the mode that only reads the
# manifest does not pay for them.
if [ "$mode" = "compare" ]; then
  for target in "${SHIPPED_TARGETS[@]}"; do
    cargo metadata --format-version 1 --locked --filter-platform "$target" \
      > "${WORK}/${target}.json"
  done
fi

python3 - "$mode" "${WORK}" "${SHIPPED_TARGETS[@]}" <<'PY'
import json
import sys

mode, work = sys.argv[1], sys.argv[2]
targets = sys.argv[3:]


def as_numbers(version):
    parts = [int(part) for part in version.split(".")]
    return tuple(parts + [0] * (3 - len(parts)))


def load(name):
    with open(f"{work}/{name}.json") as handle:
        return json.load(handle)


metadata = load("unfiltered")
workspace_members = set(metadata["workspace_members"])
members = [p for p in metadata["packages"] if p["id"] in workspace_members]

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

# Highest floor per shipped target, then the highest of those. A package unreachable on a
# target is absent from that target's resolution, so it cannot set the floor.
per_target = {}
for target in targets:
    resolved = load(target)
    reachable = [
        (package["rust_version"], package["name"])
        for package in resolved["packages"]
        if package.get("rust_version") and package["id"] not in workspace_members
    ]
    if reachable:
        per_target[target] = max(reachable, key=lambda row: as_numbers(row[0]))

if not per_target:
    print("no dependency on any shipped target declares a rust-version", file=sys.stderr)
    raise SystemExit(1)

target, (required_version, required_by) = max(
    per_target.items(), key=lambda row: as_numbers(row[1][0])
)

if as_numbers(required_version) > as_numbers(declared):
    print(
        f"workspace declares rust-version {declared}, but {required_by} needs "
        f"{required_version} on {target}\n"
        f"raise the workspace rust-version, or hold {required_by} at an older version",
        file=sys.stderr,
    )
    raise SystemExit(1)

print(f"rust-version {declared} covers every dependency reachable on a shipped target")
for name in targets:
    if name in per_target:
        version, package = per_target[name]
        print(f"  {name}: highest {version} from {package}")
print(f"all {len(members)} workspace crates declare {declared}")
PY
