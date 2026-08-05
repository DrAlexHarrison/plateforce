#!/usr/bin/env bash
# Syncs the engine into the R package, installs it from this checkout, and asks it what it
# can do.
#
# The row builds what it asks, which is what scripts/capability-surfaces.txt requires of
# every row. Asking the installed package alone reads whatever was last installed: measured
# on a checkout carrying a nineteenth refusal code in `crates/`, `plateforce::capability_json()`
# reported eighteen, and the gate called that a disagreement between the browser and R while
# the two sources agreed. The same reading passes a stale package off as a match whenever the
# drift runs the other way.
#
# It builds from the working tree. The sync defaults to the last commit, which is right for a
# release tarball and wrong here: the other two rows compile the working tree, so a run against
# an edited checkout compares three surfaces built from two different sources. Measured on the
# checkout that added a twentieth
# refusal code: the CLI and the browser reported twenty and R reported nineteen, which is the
# same manufactured split one paragraph up, arriving through the sync instead of through the
# library path.
#
# Installed into a private directory under `target/r-surface` rather than the user library,
# so concurrent gates cannot remove the package while this row reads it.
#
# The sync, the install and the two refusals it carries, that `R` on this path really is R and
# that a reported success landed a package, are scripts/r-surface.sh's whole subject, so
# it is called rather than repeated here.
#
# The sync and the install write to the other stream, because the caller reads this one as
# the surface's answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/target/r-surface"
library="$(mktemp -d "$root/target/r-surface/library.XXXXXX")"
trap 'rm -rf "$library"' EXIT

PLATEFORCE_R_LIBRARY="$library" bash "$root/scripts/r-surface.sh" --install-only >&2

env R_LIBS="$library" Rscript --vanilla -e 'cat(plateforce::capability_json())'
