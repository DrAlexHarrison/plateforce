#!/usr/bin/env bash
# R's answer to the one committed request, from a package built out of this checkout.
#
# The package is built here rather than read as found: `scripts/result-from-r.R` alone reads
# whichever `plateforce` the machine has installed. Measured on this checkout: the installed
# package answered without `registry_declared_version` while the same source built from the
# worktree answered with it, and the gate reported that as R dropping a field three other
# surfaces publish. The reading passes a stale package off as a match whenever the drift runs
# the other way, which is the more dangerous direction.
#
# The Python row builds what it asks in scripts/install-python-wheel.sh, and the capability
# gate's R row in scripts/capability-from-r.sh.
#
# Installed into a private directory under `target/r-surface` rather than the user library,
# so concurrent gates cannot remove the package while this row reads it.
#
# The build writes to the other stream, because the caller reads this one as the surface's
# answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/target/r-surface"
library="$(mktemp -d "$root/target/r-surface/library.XXXXXX")"
trap 'rm -rf "$library"' EXIT

PLATEFORCE_R_LIBRARY="$library" bash "$root/scripts/r-surface.sh" --install-only >&2

if [ ! -f "$library/plateforce/DESCRIPTION" ]; then
    echo "the R library was reported built and $library holds no package" >&2
    exit 1
fi

env R_LIBS="$library" Rscript --vanilla "$root/scripts/result-from-r.R"
