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
# `R` resolves to another program on some machines, so the interpreter is spelled by path.
# Set `R_LIBS` to install somewhere other than the default library.
#
# The sync and the install write to the other stream, because the caller reads this one as
# the surface's answer.
set -o errexit -o nounset -o pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash "$root/bindings/r/tools/sync-engine.sh" >&2
/usr/bin/R CMD INSTALL "$root/bindings/r" --no-byte-compile >&2
exec /usr/bin/Rscript -e 'cat(plateforce::capability_json())'
