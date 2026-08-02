#!/bin/sh
# Three comparisons over the toolchain floor, and the package is unsubmittable if any fails.
#
# The second one exists nowhere else in the repository: it is the comparison that decides
# whether CRAN can check this package at all, and the workspace's own script compares the
# declared floor only against the dependency tree.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")
crate="$package_root/src/rust"

declared=$(grep -v '^ *#' "$here/msrv" | grep -v '^ *$' | head -1 | tr -d ' \r')
cran=$(grep -v '^ *#' "$here/cran-rust-floor" | grep -v '^ *$' | head -1 | tr -d ' \r')
cran_read=$(sed -n 's/^# *read: *//p' "$here/cran-rust-floor" | head -1)

# Sorts 1.9 below 1.10, which a string comparison does not.
as_number() {
    echo "$1" | awk -F. '{printf "%d%03d%03d\n", $1, $2, ($3 == "" ? 0 : $3)}'
}

if [ ! -d "$crate/crates/plateforce-core" ]; then
    sh "$here/sync-engine.sh" >/dev/null
fi

# Kept apart from the comparison below so a cargo that cannot resolve the tree reports
# that, rather than handing an empty document to the reader and failing as a parse error.
if ! metadata=$(cd "$crate" && cargo metadata --format-version 1 2>&1); then
    printf 'cargo cannot resolve %s, so the dependency floor is unread\n' "$crate" >&2
    printf '%s\n' "$metadata" >&2
    exit 1
fi

highest=$(printf '%s' "$metadata" | python3 -c '
import json, sys
metadata = json.load(sys.stdin)
members = set(metadata["workspace_members"])
floors = [(p["rust_version"], p["name"], p["version"]) for p in metadata["packages"]
          if p.get("rust_version") and p["id"] not in members]
if not floors:
    print("0.0.0 nothing")
else:
    key = lambda row: tuple(int(part) for part in (row[0].split(".") + ["0", "0"])[:3])
    top = max(floors, key=key)
    print(top[0], top[1], top[2])')

highest_version=$(echo "$highest" | cut -d' ' -f1)
highest_crate=$(echo "$highest" | cut -d' ' -f2-)

stated=$(sed -n 's/.*rustc >= *\([0-9.]*\).*/\1/p' "$package_root/DESCRIPTION" | head -1)
manifest=$(sed -n 's/^rust-version = "\([0-9.]*\)"/\1/p' "$crate/Cargo.toml" | sort -u)

status=0

printf 'declared floor %s, highest dependency floor %s set by %s\n' \
    "$declared" "$highest_version" "$highest_crate"
if [ "$(as_number "$declared")" -lt "$(as_number "$highest_version")" ]; then
    printf 'the declared floor is below what %s needs\n' "$highest_crate" >&2
    status=1
fi

age_days=$(python3 -c "
import datetime, sys
read = datetime.date.fromisoformat('$cran_read')
print((datetime.date.today() - read).days)")
printf 'declared floor %s against CRAN oldest check machine %s, read %s days ago\n' \
    "$declared" "$cran" "$age_days"
if [ "$(as_number "$declared")" -gt "$(as_number "$cran")" ]; then
    printf 'CRAN cannot check a package whose floor is above %s\n' "$cran" >&2
    status=1
fi
if [ "$age_days" -gt 180 ]; then
    printf 're-read %s: a farm machine older than the reading passes this falsely\n' \
        "$(sed -n 's/^# *source: *//p' "$here/cran-rust-floor" | head -1)" >&2
    status=1
fi

printf 'declared floor %s against SystemRequirements %s\n' "$declared" "$stated"
if [ "$declared" != "$stated" ]; then
    printf 'DESCRIPTION states rustc >= %s and tools/msrv holds %s\n' "$stated" "$declared" >&2
    status=1
fi

printf 'declared floor %s against the crate manifest %s\n' "$declared" "$manifest"
if [ "$manifest" != "$declared" ]; then
    printf 'src/rust/Cargo.toml declares %s and tools/msrv holds %s\n' "$manifest" "$declared" >&2
    status=1
fi

# The comparisons above read declared versions, and a declared version says nothing
# about the language features the sources use. This one compiles. Reading alone put the
# floor at 1.76 while `plateforce-registry` used `Option::is_none_or`, stable at 1.82, so
# the SystemRequirements claim would have been false and nothing above would have said so.
if ! rustup toolchain list 2>/dev/null | grep -q "^$declared"; then
    printf 'no rustc %s installed, so the floor is a claim rather than a measurement\n' \
        "$declared" >&2
    printf 'install it with: rustup toolchain install %s --profile minimal\n' "$declared" >&2
    exit 1
fi

printf 'building the tree on rustc %s\n' "$declared"
if cd "$crate" && cargo "+$declared" build --lib --release \
        --target-dir "$crate/target/floor-$declared" >/dev/null 2>&1; then
    printf 'the tree builds on the floor it declares\n'
else
    printf 'the tree does not build on rustc %s, which is the floor it declares\n' \
        "$declared" >&2
    status=1
fi

exit "$status"
