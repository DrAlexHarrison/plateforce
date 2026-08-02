#!/bin/sh
# Copies the engine crates into the R package so the built tarball is self-contained.
#
# The three crates are not on crates.io and `cargo vendor` does not vendor a path
# dependency reached through `[patch.crates-io]`, measured 2026-08-01: the vendor
# directory came back holding every third-party crate and none of the plateforce ones.
# So an installable source package has to carry the engine sources itself. Git carries
# one copy, under `crates/`; this copy is a build artifact and is not tracked.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")
destination="$package_root/src/rust/crates"

repository=${1:-}
if [ -z "$repository" ]; then
    repository=$(CDPATH= cd -- "$package_root/../.." && pwd)
fi

for crate in plateforce-registry plateforce-core plateforce-analysis; do
    if [ ! -f "$repository/crates/$crate/Cargo.toml" ]; then
        echo "no engine source at $repository/crates/$crate" >&2
        echo "pass the repository root as the first argument" >&2
        exit 1
    fi
done

rm -rf "$destination"
mkdir -p "$destination"

for crate in plateforce-registry plateforce-core plateforce-analysis; do
    mkdir -p "$destination/$crate"
    tar -C "$repository/crates/$crate" --exclude=target -cf - . \
        | tar -C "$destination/$crate" -xf -
    printf '%s ' "$crate"
done

echo "copied from $repository/crates"
