#!/bin/sh
# One file names the binding framework, and this fails when a second one learns its name.
#
# The seam is what makes the framework choice reversible: swapping it edits `shim.rs` and
# the manifest. A name that has spread into the dispatch or into R is a swap that has
# become a rewrite, and it spreads at the moment the framework is wired rather than later.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")

status=0
seam="shim.rs"

for file in "$package_root"/src/rust/src/*.rs; do
    [ -e "$file" ] || continue
    name=$(basename "$file")
    [ "$name" = "$seam" ] && continue
    if grep -qE 'savvy|extendr' "$file"; then
        printf '%s names the binding framework\n' "src/rust/src/$name" >&2
        status=1
    fi
done

for file in "$package_root"/R/*.R; do
    [ -e "$file" ] || continue
    if grep -qE 'savvy|extendr' "$file"; then
        printf 'R/%s names the binding framework\n' "$(basename "$file")" >&2
        status=1
    fi
done

if [ "$status" -eq 0 ]; then
    printf '%s is the only file naming the framework\n' "$seam"
fi

# The boundary cost. The R that measures it lives in a file rather than in an argument to
# Rscript: a multi-line expression handed to Rscript through this shell reached Windows as
# something that faulted before it printed its first line.
if [ "${PLATEFORCE_SKIP_TIMING:-}" != "1" ]; then
    trace="$package_root/../../crates/plateforce-conformance/fixtures/subject01_trial1.force.txt"
    Rscript "$here/measure-boundary.R" "$trace" || status=1
fi

exit "$status"
