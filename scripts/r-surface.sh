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

surface_root="$repository/target/r-surface"
library="${PLATEFORCE_R_LIBRARY:-$surface_root/library}"

# Two gate runs share the vendored package source and its Cargo target. Serialise that build,
# then let each caller consume its own installed library without another run deleting it.
#
# `flock` is util-linux and macOS does not carry it. This script sets no `-e`, so where it was
# absent the shell printed `flock: command not found` and carried straight on, and two runs
# raced over one directory while the script read as though it had serialised them. `mkdir` is
# atomic on every POSIX filesystem, which is the one property a mutex needs, so it stands in.
mkdir -p "$surface_root"
if command -v flock >/dev/null 2>&1; then
    exec 9>"$surface_root/install.lock"
    flock 9
else
    lock_directory="$surface_root/install.lock.d"
    waited_seconds=0
    until mkdir "$lock_directory" 2>/dev/null; do
        # Bounded, because a run killed between taking the lock and its trap leaves the
        # directory behind, and a gate that waits for ever reads as a gate that hung.
        if [ "$waited_seconds" -ge 600 ]; then
            echo "$lock_directory has been held for ten minutes" >&2
            echo "remove it if the run that took it is gone" >&2
            exit 1
        fi
        sleep 1
        waited_seconds=$((waited_seconds + 1))
    done
    trap 'rmdir "$lock_directory" 2>/dev/null' EXIT
fi

case "$library" in
    "$surface_root"/library*) ;;
    *)
        echo "the R gate library must stay under $surface_root" >&2
        exit 1
        ;;
esac

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
    echo "$library"
    exit 0
fi

R_LIBS="$library" Rscript --vanilla bindings/r/tools/run-suite.R
status=$?
echo "R suite exit $status, library $library"
exit $status
