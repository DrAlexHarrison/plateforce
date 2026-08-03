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
# It builds from the working tree, and that is the whole of what this row asks. The sync
# defaults to the last commit, which is right for a release tarball and wrong here: the other
# two rows compile the working tree, so a run against an edited checkout compared three
# surfaces built from two different sources. Measured on the checkout that added a twentieth
# refusal code: the CLI and the browser reported twenty and R reported nineteen, which is the
# same manufactured split one paragraph up, arriving through the sync instead of through the
# library path.
#
# R is found on the path and then asked to say what it is, because a name this short is one
# some machines have given to something else, and a wrong program here would report a surface
# that does not exist. Set `R_LIBS` to install somewhere other than the default library.
#
# The sync and the install write to the other stream, because the caller reads this one as
# the surface's answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! R --version 2>/dev/null | head -1 | grep -q '^R version'; then
    echo "the R on this path is not R, so this surface could not be asked" >&2
    exit 2
fi

PLATEFORCE_SYNC_FROM=worktree bash "$root/bindings/r/tools/sync-engine.sh" >&2
R CMD INSTALL "$root/bindings/r" --no-byte-compile >&2
exec Rscript -e 'cat(plateforce::capability_json())'
