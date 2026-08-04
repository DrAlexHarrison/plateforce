#!/bin/bash
# Builds the R binding out of this working tree and runs its suite against it.
#
# Two traps live here, so the script exists rather than the four commands.
#
# The first is that `R CMD INSTALL` writes to the user library by default, so a run here would
# leave the machine's installed package built from an uncommitted tree, and every later parity
# check on any branch would read it. This installs into `target/r-surface/library` and nothing
# else, so the arm speaks for the tree it was pointed at.
#
# The second is that the R suite is the surface `cargo test --workspace` does not reach, and the
# installed package is what the parity gate reads. A worktree change is invisible to it, so the
# gate reports a real-looking divergence that is staleness. Run this before believing any R
# number.
#
# The install is not byte-compiled: this is a gate, not a shipped artefact, and the compile is
# most of the wall clock.
set -uo pipefail

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository=$(dirname -- "$here")
cd "$repository"

library="$repository/target/r-surface/library"

# `R` is a shell alias on this machine, pointing at an unrelated program that answers
# `R CMD INSTALL` with "unexpected argument 'INSTALL'". That reads like a broken R rather than a
# shadowed name. A script's own shell does not load the alias, and this asserts the binary it
# reaches is really R rather than trusting that.
if ! R --version 2>/dev/null | head -1 | grep -q '^R version'; then
    echo "the name R does not reach R here: $(command -v R)" >&2
    exit 1
fi

rm -rf "$library"
mkdir -p "$library"

# The engine crates are copied into the package rather than referenced, so the package is built
# from this tree rather than from whatever the last sync left behind.
PLATEFORCE_SYNC_FROM=worktree bash bindings/r/tools/sync-engine.sh || exit 1

R CMD INSTALL bindings/r --library="$library" --no-byte-compile || exit 1

# The install can report success and leave nothing behind, so the artefact is asked rather than
# the exit code.
if [ ! -f "$library/plateforce/DESCRIPTION" ]; then
    echo "R CMD INSTALL reported success and installed nothing into $library" >&2
    exit 1
fi

if [ "${1:-}" = "--install-only" ]; then
    echo "R library built from this tree at $library"
    exit 0
fi

R_LIBS="$library" Rscript --vanilla bindings/r/tools/run-suite.R
status=$?
echo "R suite exit $status, library $library"
exit $status
