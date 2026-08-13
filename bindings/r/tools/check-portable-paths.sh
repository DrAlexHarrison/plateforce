#!/bin/sh
# Refuses a package whose tarball would carry a path a tar reader is not required to store.
#
# `R CMD check` reads this as a NOTE on CRAN's incoming queue, and the queue costs weeks to
# re-enter. The limit is the ustar header's 100-byte name field, and this project names a
# file after the sentence it proves, so the two conventions meet here rather than anywhere
# a reader would look. Run after `tools/sync-engine.sh`, which is what puts the engine's
# deep `src/slots/` tree inside the package and moves every path a prefix longer.
#
# The margin is printed whether it passes or fails, because a file arriving one byte under
# the limit and a file arriving forty under it are different situations and only one of
# them is about to break.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
package_root=$(dirname -- "$here")
cd "$package_root"

# The name a member takes inside the tarball, which is the package directory plus the path
# relative to it. Measuring the path on disk instead would under-count by that prefix and
# report a package as portable that is not.
package=$(sed -n 's/^Package: *//p' DESCRIPTION | head -1)
readonly LIMIT=100

listing=$(find . -type f -not -path './src/rust/target/*' -not -path './src/rust/vendor/*' \
    | sed "s|^\./|$package/|" | LC_ALL=C sort -u)

counted=$(printf '%s\n' "$listing" | grep -c .)
if [ "$counted" -lt 20 ]; then
    echo "only $counted files to measure: run tools/sync-engine.sh first" >&2
    exit 1
fi

over=$(printf '%s\n' "$listing" | awk -v limit="$LIMIT" 'length($0) > limit')
longest=$(printf '%s\n' "$listing" | awk '{ print length($0), $0 }' | sort -rn | sed -n '1,3p')

printf '%s files measured, limit %s bytes\n' "$counted" "$LIMIT"
printf 'closest to the limit:\n%s\n' "$longest"

if [ -n "$over" ]; then
    count=$(printf '%s\n' "$over" | grep -c .)
    printf '%s path(s) longer than %s bytes, which a tarball is not required to store:\n' \
        "$count" "$LIMIT" >&2
    printf '  %s\n' "$over" >&2
    echo "Shorten the name, or stop copying that tree into the package." >&2
    exit 1
fi
