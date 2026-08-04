#!/usr/bin/env bash
# R's answer to the one committed request, from a package built out of this checkout.
#
# The row used to be `Rscript --vanilla scripts/result-from-r.R`, which reads whichever
# `plateforce` the machine has installed. Measured on this checkout: the installed package
# answered without `registry_declared_version` while the same source built from the worktree
# answered with it, and the gate reported that as R dropping a field three other surfaces
# publish. The reading passes a stale package off as a match whenever the drift runs the
# other way, which is the more dangerous direction.
#
# The Python row already builds what it asks, in scripts/install-python-wheel.sh, and the
# capability gate's R row already builds what it asks, in scripts/capability-from-r.sh, whose
# header records the same wall being hit twice. This row was the one left reading whatever was
# last installed.
#
# Installed into `target/r-surface/library` rather than the user library, so a gate run does
# not leave the machine's own `plateforce` built from an uncommitted tree. That is
# scripts/r-surface.sh's whole subject, so it is called rather than repeated here.
#
# The build writes to the other stream, because the caller reads this one as the surface's
# answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash "$root/scripts/r-surface.sh" --install-only >&2

library="$root/target/r-surface/library"
if [ ! -f "$library/plateforce/DESCRIPTION" ]; then
    echo "the R library was reported built and $library holds no package" >&2
    exit 1
fi

exec env R_LIBS="$library" Rscript --vanilla "$root/scripts/result-from-r.R"
